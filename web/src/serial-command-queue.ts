export class SerialCommandQueue {
  #tail: Promise<void> = Promise.resolve();

  public enqueue<T>(command: () => T | Promise<T>): Promise<T> {
    const result = this.#tail.then(command);
    this.#tail = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  }
}
