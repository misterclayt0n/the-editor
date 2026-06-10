const BENCHMARK_LINE_COUNT = 100_000;

export function makeBenchmarkText(lineCount = BENCHMARK_LINE_COUNT): string {
  const lines = new Array<string>(lineCount);
  for (let i = 0; i < lineCount; i++) {
    const padded = String(i + 1).padStart(6, "0");
    lines[i] = `const value_${padded} = "benchmark line ${padded}"; // ${i % 10 === 0 ? "checkpoint" : "payload"}`;
  }
  return lines.join("\n");
}
