class StringDecoder {
  #decoder: TextDecoder;

  constructor(encoding = "utf-8") {
    this.#decoder = new TextDecoder(encoding);
  }

  write(input: Uint8Array): string {
    return this.#decoder.decode(input, { stream: true });
  }

  end(input?: Uint8Array): string {
    if (input) return this.#decoder.decode(input);
    return this.#decoder.decode();
  }
}

const stringDecoderModule = { StringDecoder };

export { StringDecoder };
export default stringDecoderModule;
