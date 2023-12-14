function run(code, file) {
  if (file) code += '\n//@ sourceURL=' + file;
  with (this) eval(code);
}
