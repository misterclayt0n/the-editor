const ESC = "\x1b";
const ST = `${ESC}\\`;

const hexPair = (color: string, offset: number): string => color.slice(offset, offset + 2);

const xtermRgb = (color: string): string => {
  const hex = color[0] === "#" && color.length >= 7 ? color : "#000000";
  const r = hexPair(hex, 1);
  const g = hexPair(hex, 3);
  const b = hexPair(hex, 5);
  return `rgb:${r}${r}/${g}${g}/${b}${b}`;
};

const oscResponse = (code: string, color: string): string => `${ESC}]${code};${xtermRgb(color)}${ST}`;

/**
 * ghostty-web 0.4 answers DSR, but not DA/OSC color queries. Nushell probes
 * those during startup and waits for its query timeout when nothing replies.
 */
export function ghosttyUnsupportedQueryResponses(
  data: string,
  theme: Readonly<Record<string, string>>,
): string {
  let response = "";
  if (data.includes(`${ESC}[c`) || data.includes(`${ESC}[0c`)) response += `${ESC}[?1;2c`;
  if (data.includes(`${ESC}[>c`) || data.includes(`${ESC}[>0c`)) response += `${ESC}[>0;100;0c`;
  if (data.includes(`${ESC}]10;?${ST}`) || data.includes(`${ESC}]10;?\x07`)) {
    response += oscResponse("10", theme.foreground ?? "#d4d4d4");
  }
  if (data.includes(`${ESC}]11;?${ST}`) || data.includes(`${ESC}]11;?\x07`)) {
    response += oscResponse("11", theme.background ?? "#000000");
  }
  return response;
}
