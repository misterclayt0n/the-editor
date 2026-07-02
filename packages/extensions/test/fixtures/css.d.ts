// The prebundle fixture imports CSS as text (`--loader:.css=text`), the way a
// bundled shadcn/tailwind extension carries its stylesheet.
declare module "*.css" {
  const text: string;
  export default text;
}
