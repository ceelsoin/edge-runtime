use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use clap::{ArgAction, Args};
use deno_ast::{EmitOptions, TranspileOptions};
use deno_core::{JsRuntime, PollEventLoopOptions, RuntimeOptions};
use deno_graph::ast::CapturingModuleAnalyzer;
use deno_graph::source::{LoadError, LoadOptions, LoadResponse, Loader};
use deno_graph::{BuildOptions, GraphKind, ModuleGraph};
use glob::Pattern;
use runtime_core::extensions;
use runtime_core::module_loader::EszipModuleLoader;
use runtime_core::permissions::create_permissions_container;
use url::Url;

struct CliStyle {
    enabled: bool,
}

impl CliStyle {
    fn new() -> Self {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        let dumb_term = std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false);
        Self {
            enabled: !no_color && !dumb_term,
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b[{}m{}\x1b[0m", code, text)
        } else {
            text.to_string()
        }
    }

    fn dim(&self, text: &str) -> String {
        self.paint("2", text)
    }

    fn bold(&self, text: &str) -> String {
        self.paint("1", text)
    }

    fn green(&self, text: &str) -> String {
        self.paint("32", text)
    }

    fn red(&self, text: &str) -> String {
        self.paint("31", text)
    }

    fn cyan(&self, text: &str) -> String {
        self.paint("36", text)
    }

    fn black_on_green(&self, text: &str) -> String {
        self.paint("30;42", text)
    }

    fn white_on_red(&self, text: &str) -> String {
        self.paint("37;41", text)
    }

    fn black_on_cyan(&self, text: &str) -> String {
        self.paint("30;46", text)
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct TestRunStats {
    tests_total: usize,
    tests_passed: usize,
    tests_failed: usize,
    tests_ignored: usize,
}

impl TestRunStats {
    fn executed_tests(self) -> usize {
        self.tests_passed + self.tests_failed
    }

    fn add_assign(&mut self, other: TestRunStats) {
        self.tests_total += other.tests_total;
        self.tests_passed += other.tests_passed;
        self.tests_failed += other.tests_failed;
        self.tests_ignored += other.tests_ignored;
    }
}

struct FileRunOutcome {
    stats: TestRunStats,
    error: Option<anyhow::Error>,
}

fn progress_bar(done: usize, total: usize, width: usize) -> String {
    if total == 0 {
        return "".to_string();
    }
    let filled = (done * width) / total;
    let mut out = String::with_capacity(width);
    for i in 0..width {
        if i < filled {
            out.push('=');
        } else {
            out.push('-');
        }
    }
    out
}

#[derive(Args)]
pub struct TestArgs {
    /// Path, directory or glob pattern (for example: ./tests/js/**/*.ts)
    #[arg(short, long, default_value = "./tests/js/**/*.ts")]
    path: String,

    /// Ignore path/pattern (can be used multiple times)
    #[arg(short = 'i', long = "ignore", action = ArgAction::Append)]
    ignore: Vec<String>,
}

/// A simple file-system loader for deno_graph.
struct FileLoader;

impl Loader for FileLoader {
    fn load(
        &self,
        specifier: &deno_graph::ModuleSpecifier,
        _options: LoadOptions,
    ) -> deno_graph::source::LoadFuture {
        let specifier = specifier.clone();
        Box::pin(async move {
            if specifier.scheme() == "ext" {
                if let Some(content) = load_edge_assert_module(&specifier)? {
                    return Ok(Some(LoadResponse::Module {
                        content: content.into(),
                        specifier,
                        maybe_headers: None,
                        mtime: None,
                    }));
                }
                return Ok(None);
            }

            if specifier.scheme() != "file" {
                return Ok(None);
            }

            let path = specifier.to_file_path().map_err(|()| {
                LoadError::Other(Arc::new(deno_error::JsErrorBox::generic(format!(
                    "invalid file URL: {specifier}"
                ))))
            })?;

            let content = std::fs::read(&path).map_err(|e| {
                LoadError::Other(Arc::new(deno_error::JsErrorBox::generic(format!(
                    "failed to read '{}': {e}",
                    path.display()
                ))))
            })?;

            let content = rewrite_edge_assert_imports(content)?;

            Ok(Some(LoadResponse::Module {
                content: content.into(),
                specifier,
                maybe_headers: None,
                mtime: None,
            }))
        })
    }
}

fn rewrite_edge_assert_imports(content: Vec<u8>) -> Result<Vec<u8>, LoadError> {
    let source = String::from_utf8_lossy(&content).to_string();
    if !source.contains("edge://assert/") {
        return Ok(content);
    }

    let cwd = std::env::current_dir().map_err(|e| {
        LoadError::Other(Arc::new(deno_error::JsErrorBox::generic(format!(
            "failed to resolve current dir for edge:assert rewrite: {e}"
        ))))
    })?;

    let user_mod_path = cwd.join("crates/runtime-core/src/assert/user_mod.ts");
    let assert_path = cwd.join("crates/runtime-core/src/assert/assert.ts");

    let user_mod_url = Url::from_file_path(&user_mod_path)
        .map_err(|()| LoadError::Other(Arc::new(deno_error::JsErrorBox::generic(
            format!("failed to convert '{}' to file URL", user_mod_path.display())
        ))))?;
    let assert_url = Url::from_file_path(&assert_path)
        .map_err(|()| LoadError::Other(Arc::new(deno_error::JsErrorBox::generic(
            format!("failed to convert '{}' to file URL", assert_path.display())
        ))))?;

    let rewritten = source
        .replace("edge://assert/mod.ts", user_mod_url.as_str())
        .replace("edge://assert/assert.ts", assert_url.as_str());

    Ok(rewritten.into_bytes())
}

fn load_edge_assert_module(
    specifier: &deno_graph::ModuleSpecifier,
) -> Result<Option<Vec<u8>>, LoadError> {
    let relative_path = match specifier.as_str() {
        "edge://assert/mod.ts" => {
            return Ok(Some(
                b"export * from 'edge://assert/assert.ts';\n"
                    .to_vec(),
            ));
        }
        "edge://assert/assert.ts" => "crates/runtime-core/src/assert/assert.ts",
        "ext:edge_assert/mod.ts" => "crates/runtime-core/src/assert/mod.ts",
        "ext:edge_assert/assert.ts" => "crates/runtime-core/src/assert/assert.ts",
        "ext:edge_assert/mock/mod.ts" => "crates/runtime-core/src/assert/mock/mod.ts",
        "ext:edge_assert/mock/mockFn.ts" => "crates/runtime-core/src/assert/mock/mockFn.ts",
        "ext:edge_assert/mock/spy.ts" => "crates/runtime-core/src/assert/mock/spy.ts",
        "ext:edge_assert/mock/fetch.ts" => "crates/runtime-core/src/assert/mock/fetch.ts",
        "ext:edge_assert/mock/time.ts" => "crates/runtime-core/src/assert/mock/time.ts",
        _ => return Ok(None),
    };

    let cwd = std::env::current_dir().map_err(|e| {
        LoadError::Other(Arc::new(deno_error::JsErrorBox::generic(format!(
            "failed to resolve current dir for ext modules: {e}"
        ))))
    })?;

    let module_path = cwd.join(relative_path);
    let content = std::fs::read(&module_path).map_err(|e| {
        LoadError::Other(Arc::new(deno_error::JsErrorBox::generic(format!(
            "failed to read '{}': {e}",
            module_path.display()
        ))))
    })?;

    Ok(Some(content))
}

pub fn run(args: TestArgs) -> Result<(), anyhow::Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let style = CliStyle::new();
        let started_all = Instant::now();
        let files = discover_test_files(&args.path, &args.ignore)?;

        if files.is_empty() {
            return Err(anyhow::anyhow!(
                "no test files found for '{}' (ignore: {:?})",
                args.path,
                args.ignore
            ));
        }

        println!(
            "{} {}",
            style.black_on_cyan(" RUN "),
            style.bold(&format!("Running {} JS/TS test file(s)", files.len()))
        );

        let mut passed = 0usize;
        let mut failed = 0usize;
        let mut failures: Vec<(String, String)> = Vec::new();
        let mut aggregated_test_stats = TestRunStats::default();
        let total = files.len();

        for file in files {
            let label = file.display().to_string();
            print!("\n{} {}", style.cyan("RUNS"), style.dim(&label));
            let _ = std::io::stdout().flush();
            let started_file = Instant::now();

            match run_single_test_file(&file).await {
                Ok(outcome) => {
                    aggregated_test_stats.add_assign(outcome.stats);

                    if let Some(err) = outcome.error {
                        failed += 1;
                        let elapsed = started_file.elapsed().as_secs_f64();
                        let err_text = err.to_string();
                        failures.push((label.clone(), err_text.clone()));
                        eprintln!(
                            "\n{} {} {}",
                            style.white_on_red(" FAIL "),
                            style.bold(&label),
                            style.dim(&format!("({:.2}s)", elapsed))
                        );
                        eprintln!("{} {}", style.red("  ●"), err_text);
                    } else {
                        passed += 1;
                        let elapsed = started_file.elapsed().as_secs_f64();
                        println!(
                            "\n{} {} {}",
                            style.black_on_green(" PASS "),
                            style.bold(&label),
                            style.dim(&format!("({:.2}s)", elapsed))
                        );
                    }
                }
                Err(err) => {
                    failed += 1;
                    let elapsed = started_file.elapsed().as_secs_f64();
                    let err_text = err.to_string();
                    failures.push((label.clone(), err_text.clone()));
                    eprintln!(
                        "\n{} {} {}",
                        style.white_on_red(" FAIL "),
                        style.bold(&label),
                        style.dim(&format!("({:.2}s)", elapsed))
                    );
                    eprintln!("{} {}", style.red("  ●"), err_text);
                }
            }

            let done = passed + failed;
            let pct = (done * 100) / total.max(1);
            println!(
                "{} {} {}% ({}/{})",
                style.dim("progress"),
                progress_bar(done, total, 20),
                pct,
                done,
                total
            );
        }

        let total_time = started_all.elapsed().as_secs_f64();
        println!();
        println!(
            "{}: {} total, {} passed, {} failed",
            style.bold("Test Suites"),
            total,
            style.green(&passed.to_string()),
            if failed > 0 {
                style.red(&failed.to_string())
            } else {
                style.green(&failed.to_string())
            }
        );
        println!(
            "{}: {} total, {} executed, {} passed, {} failed, {} ignored",
            style.bold("Tests"),
            aggregated_test_stats.tests_total,
            aggregated_test_stats.executed_tests(),
            style.green(&aggregated_test_stats.tests_passed.to_string()),
            if aggregated_test_stats.tests_failed > 0 {
                style.red(&aggregated_test_stats.tests_failed.to_string())
            } else {
                style.green(&aggregated_test_stats.tests_failed.to_string())
            },
            style.dim(&aggregated_test_stats.tests_ignored.to_string())
        );
        println!("{}: {:.2}s", style.bold("Time"), total_time);

        if !failures.is_empty() {
            println!("\n{}", style.bold("Failures:"));
            for (idx, (file, err)) in failures.iter().enumerate() {
                println!("{} {}", style.red(&format!("{}. {}", idx + 1, file)), style.dim(""));
                println!("   {}", err);
            }
        }

        if failed > 0 {
            Err(anyhow::anyhow!("{} test file(s) failed", failed))
        } else {
            Ok(())
        }
    })
}

fn discover_test_files(path_or_pattern: &str, ignore_patterns: &[String]) -> Result<Vec<PathBuf>, anyhow::Error> {
    let cwd = std::env::current_dir()?;
    let candidate = Path::new(path_or_pattern);

    let mut files = if is_glob_pattern(path_or_pattern) {
        collect_glob_matches(path_or_pattern)?
    } else if candidate.is_dir() {
        walk_directory_for_tests(candidate)
    } else if candidate.is_file() {
        vec![candidate.to_path_buf()]
    } else {
        return Err(anyhow::anyhow!(
            "path '{}' does not exist and is not a valid glob pattern",
            path_or_pattern
        ));
    };

    let ignore_matchers = compile_patterns(ignore_patterns)?;

    files.retain(|path| is_supported_test_file(path) && !matches_ignore(path, &cwd, &ignore_matchers));

    files.sort();
    files.dedup();

    Ok(files)
}

fn is_glob_pattern(input: &str) -> bool {
    input.contains('*') || input.contains('?') || input.contains('[') || input.contains('{')
}

fn collect_glob_matches(pattern: &str) -> Result<Vec<PathBuf>, anyhow::Error> {
    let mut matches = Vec::new();
    for entry in glob::glob(pattern).map_err(|e| anyhow::anyhow!("invalid glob pattern '{}': {e}", pattern))? {
        let path = entry.map_err(|e| anyhow::anyhow!("glob read error: {e}"))?;
        if path.is_file() {
            matches.push(path);
        }
    }
    Ok(matches)
}

fn walk_directory_for_tests(dir: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn is_supported_test_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("ts") | Some("js") | Some("mts") | Some("mjs")
    )
}

fn compile_patterns(patterns: &[String]) -> Result<Vec<Pattern>, anyhow::Error> {
    patterns
        .iter()
        .map(|p| Pattern::new(p).map_err(|e| anyhow::anyhow!("invalid ignore pattern '{}': {e}", p)))
        .collect()
}

fn matches_ignore(path: &Path, cwd: &Path, ignore_patterns: &[Pattern]) -> bool {
    if ignore_patterns.is_empty() {
        return false;
    }

    let relative = path.strip_prefix(cwd).unwrap_or(path);
    let relative_str = relative.to_string_lossy();
    let relative_with_dot = format!("./{}", relative_str);

    ignore_patterns.iter().any(|pattern| {
        pattern.matches_path(path)
            || pattern.matches_path(relative)
            || pattern.matches(&relative_str)
            || pattern.matches(&relative_with_dot)
    })
}

fn read_test_stats(js_runtime: &mut JsRuntime) -> TestRunStats {
    let script = r#"(() => {
      // deno-lint-ignore no-explicit-any
      const stats = (globalThis).__edgeTestStats;
      if (!stats) return "0,0,0,0";
      return [
        Number(stats.testsTotal ?? 0),
        Number(stats.testsPassed ?? 0),
        Number(stats.testsFailed ?? 0),
        Number(stats.testsIgnored ?? 0),
      ].join(",");
    })()"#;

    let Ok(value) = js_runtime.execute_script("<test-stats>", script) else {
        return TestRunStats::default();
    };

    deno_core::scope!(scope, js_runtime);
    let local = value.open(scope);
    let Some(text) = local.to_string(scope) else {
        return TestRunStats::default();
    };
    let stats_text = text.to_rust_string_lossy(scope);
    let parts: Vec<&str> = stats_text.split(',').collect();
    if parts.len() != 4 {
        return TestRunStats::default();
    }

    let parse = |idx: usize| -> usize {
        parts.get(idx).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0)
    };

    TestRunStats {
        tests_total: parse(0),
        tests_passed: parse(1),
        tests_failed: parse(2),
        tests_ignored: parse(3),
    }
}

async fn run_single_test_file(file_path: &Path) -> Result<FileRunOutcome, anyhow::Error> {
    let entrypoint = file_path
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("cannot resolve '{}': {e}", file_path.display()))?;

    let root_url = Url::from_file_path(&entrypoint)
        .map_err(|()| anyhow::anyhow!("cannot convert path to URL: {}", entrypoint.display()))?;

    let loader = FileLoader;
    let analyzer = CapturingModuleAnalyzer::default();

    let mut graph = ModuleGraph::new(GraphKind::CodeOnly);
    graph
        .build(
            vec![root_url.clone()],
            vec![],
            &loader,
            BuildOptions {
                module_analyzer: &analyzer,
                ..Default::default()
            },
        )
        .await;

    graph.valid().map_err(|e| anyhow::anyhow!("module graph error: {e}"))?;

    let eszip = eszip::EszipV2::from_graph(eszip::FromGraphOptions {
        graph,
        parser: analyzer.as_capturing_parser(),
        module_kind_resolver: Default::default(),
        transpile_options: TranspileOptions::default(),
        emit_options: EmitOptions::default(),
        relative_file_base: None,
        npm_packages: None,
        npm_snapshot: Default::default(),
    })?;

    let module_loader = Rc::new(EszipModuleLoader::new(Arc::new(eszip)));

    let mut opts = RuntimeOptions {
        module_loader: Some(module_loader),
        extensions: extensions::get_extensions(),
        ..Default::default()
    };
    extensions::set_extension_transpiler(&mut opts);

    let mut js_runtime = JsRuntime::new(opts);
    {
        let op_state = js_runtime.op_state();
        op_state.borrow_mut().put(create_permissions_container());
    }

    let file_path_js = format!(
        "globalThis.__edgeTestFilePath = {:?};",
        entrypoint.to_string_lossy().to_string()
    );
    js_runtime.execute_script("<set-test-file>", file_path_js)?;

    let module_id = js_runtime.load_main_es_module(&root_url).await?;
    let eval_result = js_runtime.mod_evaluate(module_id);

    js_runtime
        .run_event_loop(PollEventLoopOptions {
            wait_for_inspector: false,
            pump_v8_message_loop: true,
        })
        .await?;

    let eval_outcome = eval_result.await;
    let stats = read_test_stats(&mut js_runtime);

    match eval_outcome {
        Ok(()) => Ok(FileRunOutcome { stats, error: None }),
        Err(err) => Ok(FileRunOutcome {
            stats,
            error: Some(err.into()),
        }),
    }
}
