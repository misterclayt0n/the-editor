const binding = require("./build/Release/grammar_loader.node");

/**
 * Load a raw tree-sitter grammar shared library (helix-built .so) and wrap
 * it as a node-tree-sitter-compatible language object.
 *
 * @param {string} path absolute path to the shared library
 * @param {string} symbol exported function, e.g. "tree_sitter_rust"
 * @returns {{ language: unknown, abiVersion: number }}
 */
module.exports.loadLanguage = (path, symbol) => binding.load(path, symbol);
