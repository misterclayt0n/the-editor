/*
 * dlopen-based tree-sitter grammar loader.
 *
 * Helix-built grammars are raw shared libraries exporting
 * `TSLanguage *tree_sitter_<name>(void)` — they are NOT node modules, so
 * require() cannot load them. This addon dlopens the library, calls the
 * symbol, and wraps the TSLanguage pointer exactly the way node-tree-sitter
 * expects: an object whose `language` property is a napi external tagged
 * with node-tree-sitter's LANGUAGE_TYPE_TAG (a stable blake2 of
 * "tree-sitter"/"language" — see node-tree-sitter src/language.cc).
 *
 * The dlopen handle is intentionally never closed: grammars live for the
 * process lifetime.
 */

#include <node_api.h>
#include <dlfcn.h>
#include <stdint.h>

static const napi_type_tag LANGUAGE_TYPE_TAG = {
  0x8AF2E5212AD58ABFULL, 0xD5006CAD83ABBA16ULL
};

static napi_value Load(napi_env env, napi_callback_info info) {
  size_t argc = 2;
  napi_value argv[2];
  napi_get_cb_info(env, info, &argc, argv, NULL, NULL);
  if (argc < 2) {
    napi_throw_error(env, NULL, "load(path, symbol) requires two arguments");
    return NULL;
  }

  char path[4096];
  char symbol[512];
  size_t length;
  if (napi_get_value_string_utf8(env, argv[0], path, sizeof(path), &length) != napi_ok ||
      napi_get_value_string_utf8(env, argv[1], symbol, sizeof(symbol), &length) != napi_ok) {
    napi_throw_error(env, NULL, "load(path, symbol) requires string arguments");
    return NULL;
  }

  void *handle = dlopen(path, RTLD_NOW | RTLD_LOCAL);
  if (handle == NULL) {
    napi_throw_error(env, NULL, dlerror());
    return NULL;
  }
  const void *(*language_fn)(void) = (const void *(*)(void))dlsym(handle, symbol);
  if (language_fn == NULL) {
    napi_throw_error(env, NULL, "grammar symbol not found");
    return NULL;
  }
  const void *language = language_fn();
  if (language == NULL) {
    napi_throw_error(env, NULL, "grammar function returned NULL");
    return NULL;
  }

  napi_value external;
  napi_create_external(env, (void *)language, NULL, NULL, &external);
  napi_type_tag_object(env, external, &LANGUAGE_TYPE_TAG);

  /* The ABI version is the first uint32 of the TSLanguage struct; expose it
   * so callers can reject incompatible grammars gracefully. */
  uint32_t version = *(const uint32_t *)language;

  napi_value result, version_value;
  napi_create_object(env, &result);
  napi_set_named_property(env, result, "language", external);
  napi_create_uint32(env, version, &version_value);
  napi_set_named_property(env, result, "abiVersion", version_value);
  return result;
}

NAPI_MODULE_INIT(/* env, exports */) {
  napi_value load_fn;
  napi_create_function(env, "load", NAPI_AUTO_LENGTH, Load, NULL, &load_fn);
  napi_set_named_property(env, exports, "load", load_fn);
  return exports;
}
