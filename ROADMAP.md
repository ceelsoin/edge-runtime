# Roadmap de Correções — Deno Edge Runtime

> Baseado na auditoria de segurança e arquitetura realizada em 05/03/2026.
> Cada item referencia o finding correspondente no `AUDIT.md`.
>
> Última atualização: 06/03/2026 (base em `git log` + `git diff`).
> Commits de referência: `92aa473`, `6607a2b`, `4933dda`.
> Inclui também mudanças locais ainda não commitadas em `functions/runtime-core`.

---

## Fase 0 — Crítico (Pré-Produção)

> Itens que **bloqueiam** qualquer uso em produção. Devem ser resolvidos antes de expor o runtime a tráfego externo.

### 0.1 Implementar TLS de Verdade

**Ref:** AUDIT §1.1
**Crate:** `server`
**Arquivo:** `crates/server/src/lib.rs`

- [x] Usar o `tls_acceptor` retornado por `build_tls_acceptor()` para envolver o TCP stream
- [x] Chamar `tls_acceptor.accept(stream).await` antes de passar para hyper
- [x] Servir plain HTTP apenas se TLS config não for fornecida
- [ ] Adicionar teste E2E com conexão TLS real (self-signed cert)
- [ ] Logar warning se servidor iniciar sem TLS

**Status:** 🚧 Em progresso (3/5)

**Detalhes de implementação:**
```rust
// No accept loop:
let io = if let Some(ref acceptor) = tls_acceptor {
    let tls_stream = acceptor.accept(stream).await?;
    TokioIo::new(tls_stream)
} else {
    TokioIo::new(stream)
};
```

---

### 0.2 Autenticação nos Endpoints `/_internal`

**Ref:** AUDIT §1.2
**Crate:** `server`
**Arquivo:** `crates/server/src/router.rs`

- [x] Definir variável de ambiente `EDGE_RUNTIME_API_KEY` (ou flag CLI `--api-key`)
- [x] Extrair campo `api_key: Option<String>` no `ServerConfig`
- [x] No `handle_internal()`, verificar header `X-API-Key` contra o valor configurado
- [x] Retornar `401 Unauthorized` se key ausente/incorreta
- [x] Se nenhuma key configurada, logar warning e aceitar (modo dev)
- [x] Adicionar testes unitários para auth success/failure/missing

**Status:** ✅ Concluído

**Implementação:**
- Arquitetura de dual-listener separando admin (porta 9000) e ingress (porta 8080 ou Unix socket)
- Admin router com autenticação via header `X-API-Key`
- Ingress router rejeita `/_internal/*` com 404
- Suporte a Unix socket para ingress
- Novos arquivos: `admin_router.rs`, `ingress_router.rs`

---

### 0.3 Bloquear SSRF (IPs Privados no `fetch`)

**Ref:** AUDIT §1.3
**Crate:** `runtime-core`
**Arquivo:** `crates/runtime-core/src/permissions.rs`

- [x] Implementar bloqueio de IPs privados (equivalente ao `is_private_ip`) via denylist de ranges
  - `127.0.0.0/8` (loopback)
  - `10.0.0.0/8` (RFC 1918)
  - `172.16.0.0/12` (RFC 1918)
  - `192.168.0.0/16` (RFC 1918)
  - `169.254.0.0/16` (link-local / metadata de cloud)
  - `0.0.0.0/8`
  - `::1`, `fc00::/7`, `fe80::/10` (IPv6 equivalentes)
- [x] Adicionar `deny_net` com esses ranges na `create_permissions_container()`
- [x] Manter `allow_net: Some(vec![])` para hosts públicos
- [ ] Adicionar testes que confirmem bloqueio de `fetch("http://169.254.169.254/...")`
- [ ] Adicionar testes que confirmem que `fetch("https://api.github.com/")` funciona

**Status:** 🚧 Em progresso (3/5)

---

### 0.4 Limitar Tamanho de Request/Response Body

**Ref:** AUDIT §1.4
**Crate:** `server`
**Arquivo:** `crates/server/src/router.rs`

- [x] Definir limites default para request/response (5 MiB / 10 MiB), configuráveis via CLI/env
- [x] Antes de coletar body, verificar `Content-Length` header
- [x] Se `Content-Length > MAX`, retornar `413 Payload Too Large` imediatamente
- [x] Após iniciar coleta, impor limite de leitura também sem `Content-Length` (`http_body_util::Limited`)
- [x] Definir `MAX_RESPONSE_BODY_BYTES` (default: 10 MiB) no handler
- [ ] Truncar error messages em logs para max 1 KiB
- [x] Adicionar testes com payloads oversized

**Status:** 🚧 Em progresso (6/7)

---

### 0.5 Limitar Conexões Simultâneas

**Ref:** AUDIT §2.1
**Crate:** `server`
**Arquivo:** `crates/server/src/lib.rs`

- [x] Adicionar `max_connections: usize` ao `ServerConfig` (default: 10.000)
- [x] Criar `tokio::sync::Semaphore` com o limite configurado
- [x] Adquirir permit antes de `tokio::spawn` no accept loop
- [x] Se sem permits disponíveis, dropar a conexão com log warning
- [x] Adicionar flag CLI `--max-connections`

**Status:** ✅ Concluído

```rust
let semaphore = Arc::new(Semaphore::new(config.max_connections));

// No accept loop:
let permit = semaphore.clone().try_acquire_owned();
match permit {
    Ok(permit) => {
        tokio::spawn(async move {
            let _permit = permit; // Dropped no fim da conexão
            // ... serve connection
        });
    }
    Err(_) => {
        warn!("connection limit reached, dropping connection from {}", peer_addr);
        drop(stream);
    }
}
```

---

## Fase 1 — Alta Prioridade (Semana 1-2)

> Itens que previnem crashes, resource exhaustion e comportamento incorreto.

### 1.1 Request Timeout no Isolate

**Ref:** AUDIT §2.5
**Crate:** `functions`
**Arquivo:** `crates/functions/src/lifecycle.rs`

- [ ] Envolver `handler::dispatch_request()` com `tokio::time::timeout()`
- [x] Usar `config.wall_clock_timeout_ms` como timeout
- [ ] Retornar HTTP 504 Gateway Timeout quando exceder
- [x] Logar timeout com nome da função e duração
- [x] Incrementar `metrics.total_errors` em timeout
- [x] Adicionar teste com handler que faz `while(true) {}`

**Status:** 🚧 Em progresso (4/6)

---

### 1.2 Near-Heap-Limit Callback no V8

**Ref:** AUDIT §2.3
**Crate:** `functions`
**Arquivo:** `crates/functions/src/lifecycle.rs`

- [x] Registrar `v8::Isolate::add_near_heap_limit_callback()` na criação do isolate
- [x] No callback, logar warning e retornar `current_heap + small_delta` (última chance)
- [x] Se chamado segunda vez, terminar o isolate
- [ ] Marcar função como `Error` no registry
- [ ] Adicionar teste com código que aloca memória infinitamente

**Status:** 🚧 Em progresso (3/5)

---

### 1.3 Recovery de Panic no Isolate

**Ref:** AUDIT §2.4
**Crate:** `functions`
**Arquivo:** `crates/functions/src/lifecycle.rs`

- [x] Detectar isolate morto e evitar roteamento para handle inválido (`IsolateHandle::alive`)
- [ ] Após `catch_unwind` capturar panic, atualizar status para `Error` no registry
- [ ] Fechar o `request_tx` channel para que requests pendentes recebam erro
- [ ] Implementar auto-restart com backoff exponencial (1s, 2s, 4s, 8s, max 60s)
- [ ] Limitar número de restarts consecutivos (max 5)
- [ ] Logar cada restart com counter
- [ ] Adicionar teste de panic seguido de request

**Status:** 🚧 Em progresso (1/7)

---

### 1.4 Reset do CPU Timer por Request

**Ref:** AUDIT §2.6
**Crate:** `runtime-core`
**Arquivo:** `crates/runtime-core/src/cpu_timer.rs`

- [x] Adicionar método `reset` que zera `accumulated_ms` e `exceeded`
- [ ] Chamar `reset()` antes de cada `dispatch_request`
- [x] Adicionar teste cobrindo reuso do mesmo timer após reset

**Status:** 🚧 Em progresso (2/3)

---

### 1.5 Validar Nome de Função

**Ref:** AUDIT §3.5
**Crate:** `server`
**Arquivo:** `crates/server/src/router.rs`

- [ ] Criar função `fn is_valid_function_name(name: &str) -> bool`
- [ ] Regex: `^[a-z0-9][a-z0-9-]{0,62}$`
- [ ] Validar no deploy (`POST /_internal/functions`)
- [ ] Validar no ingress (retornar 400 se inválido)
- [ ] Adicionar testes com nomes: válidos, com `..`, com `/`, unicode, vazio, muito longo

---

### 1.6 Ativar Rate Limiter

**Ref:** AUDIT §3.1
**Crate:** `server`
**Arquivo:** `crates/server/src/lib.rs`

- [ ] Aplicar `RateLimitLayer` da middleware ao serviço HTTP se `rate_limit_rps` configurado
- [ ] Retornar `429 Too Many Requests` quando exceder
- [ ] Adicionar header `Retry-After` na resposta 429

---

## Fase 2 — Média Prioridade (Semana 3-4)

> Melhorias de robustez, observabilidade e operational safety.

### 2.1 CPU Time Real (CLOCK_THREAD_CPUTIME_ID)

**Ref:** AUDIT §2.2
**Crate:** `runtime-core`
**Arquivo:** `crates/runtime-core/src/cpu_timer.rs`

- [ ] Usar `libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID)` para medir CPU real
- [ ] Manter wall-clock como fallback em plataformas sem suporte
- [ ] Documentar diferença entre CPU time e wall-clock time
- [ ] Adicionar benchmarks comparando ambas abordagens

---

### 2.2 Graceful Shutdown Real

**Ref:** AUDIT §4.5 e §2.4
**Crates:** `server`, `functions`
**Arquivos:** `crates/server/src/lib.rs`, `crates/functions/src/registry.rs`

- [ ] No shutdown, enviar `CancellationToken` para cada isolate
- [ ] Esperar com deadline (ex: 10s) que todos os isolates terminem
- [ ] Verificar `request_tx.is_closed()` para cada função
- [ ] Após deadline, forçar clear com log warning
- [ ] Adicionar teste de shutdown com requests in-flight

---

### 2.3 Cache do Endpoint de Metrics

**Ref:** AUDIT §3.2
**Crate:** `server`
**Arquivo:** `crates/server/src/router.rs`

- [ ] Criar `MetricsCache` com TTL de 15 segundos
- [ ] Armazenar resultado de `sysinfo::System` + function metrics
- [ ] Retornar cache se não expirado
- [ ] Usar `tokio::sync::RwLock` ou `parking_lot::RwLock`

---

### 2.4 Sanitizar Error Messages para Clientes

**Ref:** AUDIT §3.8
**Crate:** `server`
**Arquivo:** `crates/server/src/router.rs`

- [ ] Criar enum `ClientError` com mensagens genéricas
- [ ] Logar stack trace internamente com `tracing::error!`
- [ ] Retornar ao cliente apenas: `{"error": "internal_error", "request_id": "..."}`
- [ ] Incluir `request_id` (UUID) para correlação

---

### 2.5 Distribuited Tracing (W3C Trace Context)

**Ref:** AUDIT §5 (observações positivas — OpenTelemetry já nas deps)
**Crate:** `server`

- [ ] Propagar headers `traceparent` e `tracestate` para dentro dos isolates
- [ ] Criar span por request com function name, status, duration
- [ ] Exportar via OTLP (já nas dependências)
- [ ] Adicionar `correlation-id` header no response

---

### 2.6 Freeze de Globals no Bootstrap

**Ref:** AUDIT §4.2
**Crate:** `runtime-core`
**Arquivo:** `crates/runtime-core/src/bootstrap.js`

- [ ] Após atribuir todas as APIs a `globalThis`, aplicar `Object.freeze()` nos critiais:
  - `fetch`, `Request`, `Response`, `Headers`
  - `crypto`, `URL`, `URLSearchParams`
  - `TextEncoder`, `TextDecoder`
  - `console`
- [ ] Testar que user code não consegue sobrescrever `globalThis.fetch`

---

### 2.7 Proteger Inspector para Localhost

**Ref:** AUDIT §3.3
**Crate:** `runtime-core`

- [ ] Forçar bind do inspector em `127.0.0.1`
- [ ] Adicionar flag `--inspect-allow-remote` para override explícito
- [ ] Documentar que inspector não deve ser usado em produção
- [ ] Logar warning se inspector ativado

---

## Fase 3 — Melhoria Contínua (Mês 2+)

> Evolução de features e hardening avançado.

### 3.1 Permissões por Função

- [ ] Cada função declara capabilities necessárias (rede, hosts específicos, APIs)
- [ ] Filtrar extensões carregadas por capability
- [ ] Criar `PermissionsContainer` imutável por função
- [ ] API de deploy aceita campo `permissions` no manifest

### 3.2 V8 Snapshot para Cold Start Rápido

- [ ] Implementar `load_from_snapshot()` (atualmente TODO)
- [ ] Validar versão do V8 no snapshot vs runtime
- [ ] Benchmark de cold start: eszip vs snapshot
- [ ] Meta: cold start < 50ms

### 3.3 Streaming de Response Body

- [ ] Substituir `bytes::Bytes` por `hyper::body::Body` streaming
- [ ] Suportar `ReadableStream` no response do user code
- [ ] Permitir Server-Sent Events e chunked transfer

### 3.4 Isolate Pooling / Reuse

- [ ] Pool de isolates quentes prontos para receber requests
- [ ] Reutilizar isolate entre requests da mesma função
- [ ] Pre-warm isolates para funções com alto tráfego
- [ ] Evict LRU quando pool estiver cheio

### 3.5 Hot-Reload de Certificado TLS

- [ ] Watch no cert/key file via `notify`
- [ ] Rotacionar `TlsAcceptor` sem restart do servidor
- [ ] Logar rotação com fingerprint do novo cert

### 3.6 HTTP/3 (QUIC)

- [ ] Avaliar `quinn` ou `h3` crate
- [ ] Suportar QUIC listeners em paralelo com TCP
- [ ] ALPN negotiation para h2/h3

### 3.7 Module Integrity (Assinatura de Bundles)

- [ ] Assinar bundles eszip com HMAC-SHA256 ou Ed25519
- [ ] Verificar assinatura no load antes de execução
- [ ] Rejeitar bundles sem assinatura válida em modo produção

### 3.8 Resolver Paths Hardcoded no CLI

**Ref:** AUDIT §3.4

- [ ] Usar variável `EDGE_RUNTIME_ROOT` ou auto-detectar via `Cargo.toml` parent walk
- [ ] Ou embutir assets no binário via `include_str!` / `include_bytes!`
- [ ] Adicionar testes que rodam de diretórios não-raiz

---

## Fase 4 — Testes de Segurança

> Testes específicos que devem existir para validar as correções acima e prevenir regressões.

### 4.1 Testes de Sandbox
- [ ] `fetch("http://127.0.0.1:...")` → bloqueado
- [ ] `fetch("http://169.254.169.254/...")` → bloqueado
- [ ] `fetch("https://httpbin.org/get")` → permitido
- [ ] `Deno.readFile("...")` → não existe / permission denied
- [ ] `Deno.env.get("...")` → não existe / permission denied
- [ ] Prototype pollution via `Object.prototype.__proto__` → sem efeito

### 4.2 Testes de Resource Limits
- [x] Teste de término forçado de execução com `while(true){}` (via `terminate_execution`)
- [ ] Handler com `while(true){}` → timeout 504
- [ ] Handler que aloca 1GB → heap limit / OOM kill
- [x] Request body oversized → 413 Payload Too Large
- [ ] 20.000 conexões simultâneas → conexões excedentes dropadas

### 4.3 Testes de Auth
- [x] `POST /_internal/functions` sem API key → 401
- [x] `POST /_internal/functions` com key errada → 401
- [x] `POST /_internal/functions` com key correta → 200
- [x] `GET /{function}/` sem key → funciona (ingress público)

### 4.4 Testes de Resiliência
- [ ] Isolate panic → status muda para Error → auto-restart
- [ ] Shutdown com request in-flight → request completa ou recebe erro
- [ ] Deploy de bundle corrompido → erro 400, não crash

---

## Métricas de Sucesso

| Métrica | Alvo |
|---|---|
| Vulnerabilidades Críticas | 0 |
| Vulnerabilidades Altas | 0 |
| Cobertura de testes de segurança | > 90% dos cenários listados |
| Cold start (eszip) | < 200ms |
| Cold start (snapshot) | < 50ms |
| Max concurrent connections | 10.000+ estável |
| Request timeout enforcement | 100% dos casos |
| Memory limit enforcement | 100% dos casos |
