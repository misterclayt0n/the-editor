// GENERATED FILE — do not edit.
// Source: helix-editor/helix languages.toml ([language-server] + per-language
// language-servers). Regenerate: pnpm --filter @the-editor/syntax codegen

export type GeneratedLanguageServer = {
  readonly command: string;
  readonly args?: readonly string[];
  readonly config?: unknown;
  readonly environment?: Readonly<Record<string, string>>;
};

export type GeneratedServerLanguage = {
  readonly name: string;
  /** LSP languageId for didOpen. */
  readonly languageId: string;
  readonly fileTypes: readonly string[];
  /** Workspace root markers. */
  readonly roots?: readonly string[];
  /** Servers in priority order with helix feature constraints. */
  readonly servers: ReadonlyArray<{
    readonly name: string;
    /** Helix only-features: the server serves ONLY these. */
    readonly only?: readonly string[];
    /** Helix except-features: the server serves everything BUT these. */
    readonly except?: readonly string[];
  }>;
};

export const generatedLanguageServers: Readonly<Record<string, GeneratedLanguageServer>> = {
  "ada-gpr-language-server": {
    "command": "ada_language_server",
    "args": [
      "--language-gpr"
    ]
  },
  "ada-language-server": {
    "command": "ada_language_server"
  },
  "als": {
    "command": "als"
  },
  "amber-lsp": {
    "command": "amber-lsp"
  },
  "ameba-ls": {
    "command": "ameba-ls"
  },
  "angular": {
    "command": "ngserver",
    "args": [
      "--stdio",
      "--tsProbeLocations",
      ".",
      "--ngProbeLocations",
      "."
    ]
  },
  "asm-lsp": {
    "command": "asm-lsp"
  },
  "awk-language-server": {
    "command": "awk-language-server"
  },
  "bash-language-server": {
    "command": "bash-language-server",
    "args": [
      "start"
    ]
  },
  "bass": {
    "command": "bass",
    "args": [
      "--lsp"
    ]
  },
  "beancount-language-server": {
    "command": "beancount-language-server"
  },
  "bicep-langserver": {
    "command": "bicep-langserver"
  },
  "biome-lsp-proxy": {
    "command": "biome",
    "args": [
      "lsp-proxy"
    ]
  },
  "bitbake-language-server": {
    "command": "bitbake-language-server"
  },
  "buf": {
    "command": "buf",
    "args": [
      "lsp",
      "serve",
      "--timeout",
      "0"
    ]
  },
  "c3-lsp": {
    "command": "c3-lsp"
  },
  "cairo-language-server": {
    "command": "cairo-language-server"
  },
  "circom-lsp": {
    "command": "circom-lsp"
  },
  "cl-lsp": {
    "command": "cl-lsp"
  },
  "clangd": {
    "command": "clangd"
  },
  "clojure-lsp": {
    "command": "clojure-lsp"
  },
  "cmake-language-server": {
    "command": "cmake-language-server"
  },
  "codeql": {
    "command": "codeql",
    "args": [
      "execute",
      "language-server",
      "--check-errors=ON_CHANGE"
    ]
  },
  "commit-lsp": {
    "command": "commit-lsp",
    "args": [
      "run"
    ]
  },
  "crystalline": {
    "command": "crystalline",
    "args": [
      "--stdio"
    ]
  },
  "cs": {
    "command": "cs",
    "args": [
      "launch",
      "--contrib",
      "smithy-language-server",
      "--",
      "0"
    ]
  },
  "csharp-ls": {
    "command": "csharp-ls"
  },
  "cuelsp": {
    "command": "cue",
    "args": [
      "lsp",
      "serve"
    ]
  },
  "dart": {
    "command": "dart",
    "args": [
      "language-server",
      "--client-id=helix"
    ]
  },
  "dhall-lsp-server": {
    "command": "dhall-lsp-server"
  },
  "djlsp": {
    "command": "djlsp"
  },
  "docker-langserver": {
    "command": "docker-langserver",
    "args": [
      "--stdio"
    ]
  },
  "docker-compose-langserver": {
    "command": "docker-compose-langserver",
    "args": [
      "--stdio"
    ]
  },
  "dot-language-server": {
    "command": "dot-language-server",
    "args": [
      "--stdio"
    ]
  },
  "dts-lsp": {
    "command": "dts-lsp"
  },
  "earthlyls": {
    "command": "earthlyls"
  },
  "eiffel-language-server": {
    "command": "eiffel-language-server"
  },
  "elixir-ls": {
    "command": "elixir-ls",
    "config": {
      "elixirLS": {
        "dialyzerEnabled": false
      }
    }
  },
  "elm-language-server": {
    "command": "elm-language-server"
  },
  "elp": {
    "command": "elp",
    "args": [
      "server"
    ]
  },
  "elvish": {
    "command": "elvish",
    "args": [
      "-lsp"
    ]
  },
  "erlang-ls": {
    "command": "erlang_ls"
  },
  "expert": {
    "command": "expert",
    "args": [
      "--stdio"
    ]
  },
  "fennel-ls": {
    "command": "fennel-ls"
  },
  "fish-lsp": {
    "command": "fish-lsp",
    "args": [
      "start"
    ]
  },
  "forc": {
    "command": "forc",
    "args": [
      "lsp"
    ]
  },
  "forth-lsp": {
    "command": "forth-lsp"
  },
  "fortls": {
    "command": "fortls",
    "args": [
      "--lowercase_intrinsics"
    ]
  },
  "fsharp-ls": {
    "command": "fsautocomplete",
    "config": {
      "AutomaticWorkspaceInit": true
    }
  },
  "gitlab-ci-ls": {
    "command": "gitlab-ci-ls"
  },
  "gleam": {
    "command": "gleam",
    "args": [
      "lsp"
    ]
  },
  "glsl_analyzer": {
    "command": "glsl_analyzer"
  },
  "glsld": {
    "command": "glsld",
    "args": [
      "--stdio"
    ]
  },
  "graphql-language-service": {
    "command": "graphql-lsp",
    "args": [
      "server",
      "-m",
      "stream"
    ]
  },
  "hare-lsp": {
    "command": "hare-lsp",
    "args": [
      "-S"
    ]
  },
  "harper-ls": {
    "command": "harper-ls",
    "args": [
      "--stdio"
    ]
  },
  "haskell-language-server": {
    "command": "haskell-language-server-wrapper",
    "args": [
      "--lsp"
    ]
  },
  "hdls": {
    "command": "hdls"
  },
  "hyprls": {
    "command": "hyprls"
  },
  "hyuga": {
    "command": "hyuga"
  },
  "idris2-lsp": {
    "command": "idris2-lsp"
  },
  "intelephense": {
    "command": "intelephense",
    "args": [
      "--stdio"
    ]
  },
  "jdtls": {
    "command": "jdtls"
  },
  "jedi": {
    "command": "jedi-language-server"
  },
  "jq-lsp": {
    "command": "jq-lsp"
  },
  "jsonnet-language-server": {
    "command": "jsonnet-language-server",
    "args": [
      "-t",
      "--lint"
    ]
  },
  "julia": {
    "command": "julia",
    "args": [
      "--startup-file=no",
      "--history-file=no",
      "--quiet",
      "-e",
      "using LanguageServer; runserver()"
    ]
  },
  "just-lsp": {
    "command": "just-lsp"
  },
  "koka": {
    "command": "koka",
    "args": [
      "--language-server",
      "--lsstdio"
    ]
  },
  "koto-ls": {
    "command": "koto-ls"
  },
  "kotlin-lsp": {
    "command": "kotlin-lsp",
    "args": [
      "--stdio"
    ]
  },
  "kotlin-language-server": {
    "command": "kotlin-language-server"
  },
  "lean": {
    "command": "lake",
    "args": [
      "serve"
    ]
  },
  "ltex-ls": {
    "command": "ltex-ls"
  },
  "ltex-ls-plus": {
    "command": "ltex-ls-plus"
  },
  "markdoc-ls": {
    "command": "markdoc-ls",
    "args": [
      "--stdio"
    ]
  },
  "markdown-oxide": {
    "command": "markdown-oxide"
  },
  "marksman": {
    "command": "marksman",
    "args": [
      "server"
    ]
  },
  "metals": {
    "command": "metals",
    "config": {
      "isHttpEnabled": true,
      "metals": {
        "inlayHints": {
          "typeParameters": {
            "enable": true
          },
          "hintsInPatternMatch": {
            "enable": true
          }
        }
      }
    }
  },
  "mesonlsp": {
    "command": "mesonlsp",
    "args": [
      "--lsp"
    ]
  },
  "mint": {
    "command": "mint",
    "args": [
      "tool",
      "ls"
    ]
  },
  "mojo-lsp-server": {
    "command": "pixi",
    "args": [
      "run",
      "mojo-lsp-server"
    ]
  },
  "neocmakelsp": {
    "command": "neocmakelsp",
    "args": [
      "stdio"
    ]
  },
  "nil": {
    "command": "nil"
  },
  "nimlangserver": {
    "command": "nimlangserver"
  },
  "nimlsp": {
    "command": "nimlsp"
  },
  "nixd": {
    "command": "nixd"
  },
  "nls": {
    "command": "nls"
  },
  "nu-lint": {
    "command": "nu-lint",
    "args": [
      "--lsp"
    ]
  },
  "nu-lsp": {
    "command": "nu",
    "args": [
      "--lsp"
    ]
  },
  "ocamllsp": {
    "command": "ocamllsp"
  },
  "ols": {
    "command": "ols"
  },
  "oxlint-language-server": {
    "command": "oxlint",
    "args": [
      "--lsp"
    ]
  },
  "omnisharp": {
    "command": "OmniSharp",
    "args": [
      "--languageserver"
    ]
  },
  "openscad-lsp": {
    "command": "openscad-lsp",
    "args": [
      "--stdio"
    ]
  },
  "pasls": {
    "command": "pasls"
  },
  "pbkit": {
    "command": "pb",
    "args": [
      "lsp"
    ]
  },
  "perlnavigator": {
    "command": "perlnavigator",
    "args": [
      "--stdio"
    ]
  },
  "pest-language-server": {
    "command": "pest-language-server"
  },
  "pkl-lsp": {
    "command": "pkl-lsp"
  },
  "pony-lsp": {
    "command": "pony-lsp",
    "args": [
      "--stdio"
    ],
    "config": {
      "defines": [],
      "ponypath": []
    }
  },
  "prisma-language-server": {
    "command": "prisma-language-server",
    "args": [
      "--stdio"
    ],
    "config": {
      "prisma": {
        "enableDiagnostics": true
      }
    }
  },
  "purescript-language-server": {
    "command": "purescript-language-server",
    "args": [
      "--stdio"
    ]
  },
  "pylsp": {
    "command": "pylsp"
  },
  "pyrefly": {
    "command": "pyrefly",
    "args": [
      "lsp"
    ]
  },
  "pyright": {
    "command": "pyright-langserver",
    "args": [
      "--stdio"
    ],
    "config": {}
  },
  "protols": {
    "command": "protols"
  },
  "basedpyright": {
    "command": "basedpyright-langserver",
    "args": [
      "--stdio"
    ],
    "config": {}
  },
  "pylyzer": {
    "command": "pylyzer",
    "args": [
      "--server"
    ]
  },
  "pytest-language-server": {
    "command": "pytest-language-server"
  },
  "qmlls": {
    "command": "qmlls"
  },
  "quint-language-server": {
    "command": "quint-language-server",
    "args": [
      "--stdio"
    ]
  },
  "r": {
    "command": "R",
    "args": [
      "--no-echo",
      "-e",
      "languageserver::run()"
    ]
  },
  "racket": {
    "command": "racket",
    "args": [
      "-l",
      "racket-langserver"
    ]
  },
  "regols": {
    "command": "regols"
  },
  "rescript-language-server": {
    "command": "rescript-language-server",
    "args": [
      "--stdio"
    ]
  },
  "ripple-lsp": {
    "command": "ripple-language-server",
    "args": [
      "--stdio"
    ]
  },
  "robotcode": {
    "command": "robotcode",
    "args": [
      "language-server",
      "--stdio"
    ]
  },
  "robotframework_ls": {
    "command": "robotframework_ls"
  },
  "ron-lsp": {
    "command": "ron-lsp"
  },
  "roslyn-language-server": {
    "command": "roslyn-language-server",
    "args": [
      "--stdio",
      "--autoLoadProjects"
    ]
  },
  "ruff": {
    "command": "ruff",
    "args": [
      "server"
    ]
  },
  "ruby-lsp": {
    "command": "ruby-lsp"
  },
  "rshtml-analyzer": {
    "command": "rshtml-analyzer",
    "args": [
      "--stdio"
    ]
  },
  "rumdl": {
    "command": "rumdl",
    "args": [
      "server"
    ]
  },
  "serve-d": {
    "command": "serve-d"
  },
  "slangd": {
    "command": "slangd"
  },
  "slint-lsp": {
    "command": "slint-lsp"
  },
  "smalisp": {
    "command": "smalisp"
  },
  "systemd-lsp": {
    "command": "systemd-lsp"
  },
  "solargraph": {
    "command": "solargraph",
    "args": [
      "stdio"
    ]
  },
  "solc": {
    "command": "solc",
    "args": [
      "--lsp"
    ]
  },
  "sourcekit-lsp": {
    "command": "sourcekit-lsp"
  },
  "spade-language-server": {
    "command": "spade-language-server"
  },
  "starpls": {
    "command": "starpls"
  },
  "styx": {
    "command": "styx",
    "args": [
      "lsp"
    ]
  },
  "svlangserver": {
    "command": "svlangserver",
    "config": {
      "systemverilog": {
        "includeIndexing": [
          "*.{v,vh,sv,svh}",
          "**/*.{v,vh,sv,svh}"
        ]
      }
    }
  },
  "swipl": {
    "command": "swipl",
    "args": [
      "-g",
      "use_module(library(lsp_server))",
      "-g",
      "lsp_server:main",
      "-t",
      "halt",
      "--",
      "stdio"
    ]
  },
  "superhtml": {
    "command": "superhtml",
    "args": [
      "lsp"
    ]
  },
  "tailwindcss-ls": {
    "command": "tailwindcss-language-server",
    "args": [
      "--stdio"
    ]
  },
  "taplo": {
    "command": "taplo",
    "args": [
      "lsp",
      "stdio"
    ]
  },
  "templ": {
    "command": "templ",
    "args": [
      "lsp"
    ]
  },
  "terraform-ls": {
    "command": "terraform-ls",
    "args": [
      "serve"
    ]
  },
  "texlab": {
    "command": "texlab"
  },
  "tilt": {
    "command": "tilt",
    "args": [
      "lsp",
      "start"
    ]
  },
  "tombi": {
    "command": "tombi",
    "args": [
      "lsp"
    ]
  },
  "ty": {
    "command": "ty",
    "args": [
      "server"
    ]
  },
  "typespec": {
    "command": "tsp-server",
    "args": [
      "--stdio"
    ]
  },
  "vala-language-server": {
    "command": "vala-language-server"
  },
  "vale-ls": {
    "command": "vale-ls"
  },
  "verible-verilog-ls": {
    "command": "verible-verilog-ls"
  },
  "vhdl_ls": {
    "command": "vhdl_ls"
  },
  "vlang-language-server": {
    "command": "v-analyzer"
  },
  "vscode-css-language-server": {
    "command": "vscode-css-language-server",
    "args": [
      "--stdio"
    ],
    "config": {
      "provideFormatter": true,
      "css": {
        "validate": {
          "enable": true
        }
      }
    }
  },
  "vscode-html-language-server": {
    "command": "vscode-html-language-server",
    "args": [
      "--stdio"
    ],
    "config": {
      "provideFormatter": true
    }
  },
  "vscode-json-language-server": {
    "command": "vscode-json-language-server",
    "args": [
      "--stdio"
    ],
    "config": {
      "provideFormatter": true,
      "json": {
        "validate": {
          "enable": true
        }
      }
    }
  },
  "vuels": {
    "command": "vue-language-server",
    "args": [
      "--stdio"
    ],
    "config": {
      "typescript": {
        "tsdk": "node_modules/typescript/lib/"
      }
    }
  },
  "wgsl-analyzer": {
    "command": "wgsl-analyzer"
  },
  "wikitext-lsp": {
    "command": "wikitext-lsp",
    "args": [
      "--stdio"
    ]
  },
  "yaml-language-server": {
    "command": "yaml-language-server",
    "args": [
      "--stdio"
    ]
  },
  "yls": {
    "command": "yls",
    "args": [
      "-vv"
    ]
  },
  "zls": {
    "command": "zls"
  },
  "blueprint-compiler": {
    "command": "blueprint-compiler",
    "args": [
      "lsp"
    ]
  },
  "tinymist": {
    "command": "tinymist"
  },
  "ts_query_ls": {
    "command": "ts_query_ls"
  },
  "termux-language-server": {
    "command": "termux-language-server"
  },
  "helm_ls": {
    "command": "helm_ls",
    "args": [
      "serve"
    ]
  },
  "ember-language-server": {
    "command": "ember-language-server",
    "args": [
      "--stdio"
    ]
  },
  "teal-language-server": {
    "command": "teal-language-server"
  },
  "wasm-language-tools": {
    "command": "wat_server"
  },
  "sourcepawn-studio": {
    "command": "sourcepawn-studio"
  },
  "luau": {
    "command": "luau-lsp",
    "args": [
      "lsp"
    ]
  },
  "ebnfer": {
    "command": "ebnfer"
  },
  "zizmor": {
    "command": "zizmor",
    "args": [
      "--lsp"
    ]
  },
  "actions-language-server": {
    "command": "actions-languageserver",
    "args": [
      "--stdio"
    ],
    "config": {
      "actions-language-server": {
        "sessionToken": ""
      }
    }
  },
  "ansible-language-server": {
    "command": "ansible-language-server",
    "args": [
      "--stdio"
    ]
  },
  "astro-ls": {
    "command": "astro-ls",
    "args": [
      "--stdio"
    ],
    "config": {
      "typescript": {
        "tsdk": "node_modules/typescript/lib"
      }
    }
  },
  "lua-language-server": {
    "command": "lua-language-server",
    "config": {
      "Lua": {
        "hint": {
          "enable": true,
          "arrayIndex": "Enable",
          "setType": true,
          "paramName": "All",
          "paramType": true,
          "await": true
        }
      }
    }
  },
  "gopls": {
    "command": "gopls",
    "config": {
      "hints": {
        "assignVariableTypes": true,
        "compositeLiteralFields": true,
        "constantValues": true,
        "functionTypeParameters": true,
        "parameterNames": true,
        "rangeVariableTypes": true
      }
    }
  },
  "golangci-lint-lsp": {
    "command": "golangci-lint-langserver",
    "config": {
      "command": [
        "golangci-lint",
        "run",
        "--output.json.path=stdout",
        "--show-stats=false",
        "--issues-exit-code=1"
      ]
    }
  },
  "rust-analyzer": {
    "command": "rust-analyzer",
    "config": {
      "inlayHints": {
        "bindingModeHints": {
          "enable": false
        },
        "closingBraceHints": {
          "minLines": 10
        },
        "closureReturnTypeHints": {
          "enable": "with_block"
        },
        "discriminantHints": {
          "enable": "fieldless"
        },
        "lifetimeElisionHints": {
          "enable": "skip_trivial"
        },
        "typeHints": {
          "hideClosureInitialization": false
        }
      },
      "files": {
        "watcher": "server"
      }
    }
  },
  "typescript-language-server": {
    "command": "typescript-language-server",
    "args": [
      "--stdio"
    ],
    "config": {
      "hostInfo": "helix",
      "typescript": {
        "inlayHints": {
          "includeInlayEnumMemberValueHints": true,
          "includeInlayFunctionLikeReturnTypeHints": true,
          "includeInlayFunctionParameterTypeHints": true,
          "includeInlayParameterNameHints": "all",
          "includeInlayParameterNameHintsWhenArgumentMatchesName": true,
          "includeInlayPropertyDeclarationTypeHints": true,
          "includeInlayVariableTypeHints": true
        }
      },
      "javascript": {
        "inlayHints": {
          "includeInlayEnumMemberValueHints": true,
          "includeInlayFunctionLikeReturnTypeHints": true,
          "includeInlayFunctionParameterTypeHints": true,
          "includeInlayParameterNameHints": "all",
          "includeInlayParameterNameHintsWhenArgumentMatchesName": true,
          "includeInlayPropertyDeclarationTypeHints": true,
          "includeInlayVariableTypeHints": true
        }
      }
    }
  },
  "svelteserver": {
    "command": "svelteserver",
    "args": [
      "--stdio"
    ],
    "config": {
      "configuration": {
        "typescript": {
          "inlayHints": {
            "parameterTypes": {
              "enabled": true
            },
            "variableTypes": {
              "enabled": true
            },
            "propertyDeclarationTypes": {
              "enabled": true
            },
            "functionLikeReturnTypes": {
              "enabled": true
            },
            "enumMemberValues": {
              "enabled": true
            },
            "parameterNames": {
              "enabled": "all"
            }
          }
        },
        "javascript": {
          "inlayHints": {
            "parameterTypes": {
              "enabled": true
            },
            "variableTypes": {
              "enabled": true
            },
            "propertyDeclarationTypes": {
              "enabled": true
            },
            "functionLikeReturnTypes": {
              "enabled": true
            },
            "enumMemberValues": {
              "enabled": true
            },
            "parameterNames": {
              "enabled": "all"
            }
          }
        }
      }
    }
  },
  "vscode-eslint-language-server": {
    "command": "vscode-eslint-language-server",
    "args": [
      "--stdio"
    ],
    "config": {
      "validate": "on",
      "experimental": {
        "useFlatConfig": false
      },
      "rulesCustomizations": [],
      "run": "onType",
      "problems": {
        "shortenToSingleLine": false
      },
      "nodePath": "",
      "codeAction": {
        "disableRuleComment": {
          "enable": true,
          "location": "separateLine"
        },
        "showDocumentation": {
          "enable": true
        }
      },
      "workingDirectory": {
        "mode": "location"
      }
    }
  },
  "clarinet": {
    "command": "clarinet",
    "args": [
      "lsp"
    ]
  },
  "docker-language-server": {
    "command": "docker-language-server",
    "args": [
      "start",
      "--stdio"
    ]
  },
  "kcl-lsp": {
    "command": "kcl-language-server",
    "args": [
      "server",
      "--stdio"
    ]
  },
  "drools-lsp": {
    "command": "drools-lsp"
  }
};

export const generatedServerLanguages: ReadonlyArray<GeneratedServerLanguage> = [
  {
    "name": "rust",
    "languageId": "rust",
    "fileTypes": [
      "rs"
    ],
    "roots": [
      "Cargo.toml",
      "Cargo.lock"
    ],
    "servers": [
      {
        "name": "rust-analyzer"
      }
    ]
  },
  {
    "name": "sway",
    "languageId": "sway",
    "fileTypes": [
      "sw"
    ],
    "roots": [
      "Forc.toml",
      "Forc.lock"
    ],
    "servers": [
      {
        "name": "forc"
      }
    ]
  },
  {
    "name": "toml",
    "languageId": "toml",
    "fileTypes": [
      "toml"
    ],
    "servers": [
      {
        "name": "taplo"
      },
      {
        "name": "tombi"
      }
    ]
  },
  {
    "name": "awk",
    "languageId": "awk",
    "fileTypes": [
      "awk",
      "gawk",
      "nawk",
      "mawk"
    ],
    "servers": [
      {
        "name": "awk-language-server"
      }
    ]
  },
  {
    "name": "protobuf",
    "languageId": "protobuf",
    "fileTypes": [
      "proto"
    ],
    "servers": [
      {
        "name": "buf"
      },
      {
        "name": "pbkit"
      },
      {
        "name": "protols"
      }
    ]
  },
  {
    "name": "eiffel",
    "languageId": "eiffel",
    "fileTypes": [
      "e"
    ],
    "servers": [
      {
        "name": "eiffel-language-server"
      }
    ]
  },
  {
    "name": "elixir",
    "languageId": "elixir",
    "fileTypes": [
      "ex",
      "exs"
    ],
    "roots": [
      "mix.exs",
      "mix.lock"
    ],
    "servers": [
      {
        "name": "elixir-ls"
      },
      {
        "name": "expert"
      }
    ]
  },
  {
    "name": "fennel",
    "languageId": "fennel",
    "fileTypes": [
      "fnl",
      "fnlm"
    ],
    "servers": [
      {
        "name": "fennel-ls"
      }
    ]
  },
  {
    "name": "fish",
    "languageId": "fish",
    "fileTypes": [
      "fish"
    ],
    "servers": [
      {
        "name": "fish-lsp"
      }
    ]
  },
  {
    "name": "mint",
    "languageId": "mint",
    "fileTypes": [
      "mint"
    ],
    "servers": [
      {
        "name": "mint"
      }
    ]
  },
  {
    "name": "mojo",
    "languageId": "mojo",
    "fileTypes": [
      "mojo",
      "🔥"
    ],
    "roots": [
      "pixi.toml",
      "pixi.lock"
    ],
    "servers": [
      {
        "name": "mojo-lsp-server"
      }
    ]
  },
  {
    "name": "json",
    "languageId": "json",
    "fileTypes": [
      "json",
      "arb",
      "ipynb",
      "geojson",
      "gltf",
      "webmanifest",
      "js.map",
      "ts.map",
      "css.map",
      "jsonl",
      "avsc",
      "ldtk",
      "ldtkl",
      "sublime-build",
      "sublime-color-scheme",
      "sublime-commands",
      "sublime-completions",
      "sublime-keymap",
      "sublime-macro",
      "sublime-menu",
      "sublime-mousemap",
      "sublime-project",
      "sublime-settings",
      "sublime-theme",
      "sublime-workspace",
      "code-workspace"
    ],
    "servers": [
      {
        "name": "vscode-json-language-server"
      }
    ]
  },
  {
    "name": "jsonc",
    "languageId": "jsonc",
    "fileTypes": [
      "jsonc"
    ],
    "servers": [
      {
        "name": "vscode-json-language-server"
      }
    ]
  },
  {
    "name": "json-ld",
    "languageId": "json-ld",
    "fileTypes": [
      "jsonld"
    ],
    "servers": [
      {
        "name": "vscode-json-language-server"
      }
    ]
  },
  {
    "name": "c",
    "languageId": "c",
    "fileTypes": [
      "c"
    ],
    "servers": [
      {
        "name": "clangd"
      }
    ]
  },
  {
    "name": "cpp",
    "languageId": "cpp",
    "fileTypes": [
      "cc",
      "hh",
      "c++",
      "cpp",
      "hpp",
      "h",
      "ipp",
      "tpp",
      "cxx",
      "hxx",
      "ixx",
      "txx",
      "ino",
      "C",
      "H",
      "cu",
      "cuh",
      "cppm",
      "h++",
      "ii",
      "inl"
    ],
    "servers": [
      {
        "name": "clangd"
      }
    ]
  },
  {
    "name": "crystal",
    "languageId": "crystal",
    "fileTypes": [
      "cr"
    ],
    "roots": [
      "shard.yml",
      "shard.lock"
    ],
    "servers": [
      {
        "name": "crystalline"
      },
      {
        "name": "ameba-ls"
      }
    ]
  },
  {
    "name": "c-sharp",
    "languageId": "c-sharp",
    "fileTypes": [
      "cs",
      "csx",
      "cake"
    ],
    "roots": [
      "*.slnx",
      "*.sln",
      "*.csproj"
    ],
    "servers": [
      {
        "name": "roslyn-language-server"
      },
      {
        "name": "omnisharp"
      },
      {
        "name": "csharp-ls"
      }
    ]
  },
  {
    "name": "c3",
    "languageId": "c3",
    "fileTypes": [
      "c3",
      "c3i",
      "c3t"
    ],
    "roots": [
      "project.json"
    ],
    "servers": [
      {
        "name": "c3-lsp"
      }
    ]
  },
  {
    "name": "go",
    "languageId": "go",
    "fileTypes": [
      "go"
    ],
    "roots": [
      "go.work",
      "go.mod"
    ],
    "servers": [
      {
        "name": "gopls"
      },
      {
        "name": "golangci-lint-lsp"
      }
    ]
  },
  {
    "name": "gotmpl",
    "languageId": "gotmpl",
    "fileTypes": [
      "gotmpl"
    ],
    "servers": [
      {
        "name": "gopls"
      }
    ]
  },
  {
    "name": "javascript",
    "languageId": "javascript",
    "fileTypes": [
      "js",
      "mjs",
      "cjs",
      "rules",
      "es6",
      "pac",
      "gs"
    ],
    "roots": [
      "package.json",
      "jsconfig.json"
    ],
    "servers": [
      {
        "name": "typescript-language-server"
      }
    ]
  },
  {
    "name": "jsx",
    "languageId": "javascriptreact",
    "fileTypes": [
      "jsx"
    ],
    "roots": [
      "package.json",
      "jsconfig.json"
    ],
    "servers": [
      {
        "name": "typescript-language-server"
      }
    ]
  },
  {
    "name": "typescript",
    "languageId": "typescript",
    "fileTypes": [
      "ts",
      "mts",
      "cts"
    ],
    "roots": [
      "package.json",
      "tsconfig.json"
    ],
    "servers": [
      {
        "name": "typescript-language-server"
      }
    ]
  },
  {
    "name": "typespec",
    "languageId": "typespec",
    "fileTypes": [
      "tsp"
    ],
    "roots": [
      "tspconfig.yaml"
    ],
    "servers": [
      {
        "name": "typespec"
      }
    ]
  },
  {
    "name": "tsx",
    "languageId": "typescriptreact",
    "fileTypes": [
      "tsx"
    ],
    "roots": [
      "package.json",
      "tsconfig.json"
    ],
    "servers": [
      {
        "name": "typescript-language-server"
      }
    ]
  },
  {
    "name": "css",
    "languageId": "css",
    "fileTypes": [
      "css"
    ],
    "servers": [
      {
        "name": "vscode-css-language-server"
      }
    ]
  },
  {
    "name": "scss",
    "languageId": "scss",
    "fileTypes": [
      "scss"
    ],
    "servers": [
      {
        "name": "vscode-css-language-server"
      }
    ]
  },
  {
    "name": "less",
    "languageId": "less",
    "fileTypes": [
      "less"
    ],
    "servers": [
      {
        "name": "vscode-css-language-server"
      }
    ]
  },
  {
    "name": "html",
    "languageId": "html",
    "fileTypes": [
      "html",
      "htm",
      "shtml",
      "xhtml",
      "xht",
      "jsp",
      "asp",
      "aspx",
      "jshtm",
      "volt",
      "rhtml",
      "cshtml"
    ],
    "servers": [
      {
        "name": "vscode-html-language-server"
      },
      {
        "name": "superhtml"
      }
    ]
  },
  {
    "name": "python",
    "languageId": "python",
    "fileTypes": [
      "py",
      "pyi",
      "py3",
      "pyw",
      "ptl",
      "rpy",
      "cpy",
      "ipy",
      "pyt"
    ],
    "roots": [
      "pyproject.toml",
      "setup.py",
      "poetry.lock",
      "pyrightconfig.json"
    ],
    "servers": [
      {
        "name": "ty"
      },
      {
        "name": "ruff"
      },
      {
        "name": "jedi"
      },
      {
        "name": "pylsp"
      }
    ]
  },
  {
    "name": "nickel",
    "languageId": "nickel",
    "fileTypes": [
      "ncl"
    ],
    "servers": [
      {
        "name": "nls"
      }
    ]
  },
  {
    "name": "nix",
    "languageId": "nix",
    "fileTypes": [
      "nix"
    ],
    "servers": [
      {
        "name": "nil"
      },
      {
        "name": "nixd"
      }
    ]
  },
  {
    "name": "ruby",
    "languageId": "ruby",
    "fileTypes": [
      "rb",
      "rake",
      "irb",
      "gemspec",
      "rabl",
      "jbuilder",
      "jb",
      "podspec",
      "rjs",
      "rbi",
      "rbs"
    ],
    "servers": [
      {
        "name": "ruby-lsp"
      },
      {
        "name": "solargraph"
      }
    ]
  },
  {
    "name": "bash",
    "languageId": "bash",
    "fileTypes": [
      "sh",
      "bash",
      "ash",
      "dash",
      "ksh",
      "mksh",
      "zsh",
      "zshenv",
      "zlogin",
      "zlogout",
      "zprofile",
      "zshrc",
      "eclass",
      "ebuild",
      "bazelrc",
      "Renviron",
      "zsh-theme",
      "cshrc",
      "tcshrc",
      "bashrc_Apple_Terminal",
      "zshrc_Apple_Terminal"
    ],
    "servers": [
      {
        "name": "bash-language-server"
      }
    ]
  },
  {
    "name": "php",
    "languageId": "php",
    "fileTypes": [
      "php",
      "inc",
      "php4",
      "php5",
      "phtml",
      "ctp"
    ],
    "roots": [
      "composer.json",
      "index.php"
    ],
    "servers": [
      {
        "name": "intelephense"
      }
    ]
  },
  {
    "name": "latex",
    "languageId": "latex",
    "fileTypes": [
      "tex",
      "dtx",
      "ins",
      "sty",
      "cls",
      "Rd",
      "bbx",
      "cbx"
    ],
    "servers": [
      {
        "name": "texlab"
      }
    ]
  },
  {
    "name": "bibtex",
    "languageId": "bibtex",
    "fileTypes": [
      "bib"
    ],
    "servers": [
      {
        "name": "texlab"
      }
    ]
  },
  {
    "name": "lean",
    "languageId": "lean",
    "fileTypes": [
      "lean"
    ],
    "roots": [
      "lakefile.lean",
      "lakefile.toml"
    ],
    "servers": [
      {
        "name": "lean"
      }
    ]
  },
  {
    "name": "julia",
    "languageId": "julia",
    "fileTypes": [
      "jl"
    ],
    "roots": [
      "Manifest.toml",
      "Project.toml"
    ],
    "servers": [
      {
        "name": "julia"
      }
    ]
  },
  {
    "name": "java",
    "languageId": "java",
    "fileTypes": [
      "java",
      "jav",
      "pde"
    ],
    "roots": [
      "pom.xml",
      "build.gradle",
      "build.gradle.kts"
    ],
    "servers": [
      {
        "name": "jdtls"
      }
    ]
  },
  {
    "name": "smali",
    "languageId": "smali",
    "fileTypes": [
      "smali"
    ],
    "servers": [
      {
        "name": "smalisp"
      }
    ]
  },
  {
    "name": "beancount",
    "languageId": "beancount",
    "fileTypes": [
      "beancount",
      "bean"
    ],
    "servers": [
      {
        "name": "beancount-language-server"
      }
    ]
  },
  {
    "name": "ocaml",
    "languageId": "ocaml",
    "fileTypes": [
      "ml"
    ],
    "servers": [
      {
        "name": "ocamllsp"
      }
    ]
  },
  {
    "name": "ocaml-interface",
    "languageId": "ocaml-interface",
    "fileTypes": [
      "mli"
    ],
    "servers": [
      {
        "name": "ocamllsp"
      }
    ]
  },
  {
    "name": "lua",
    "languageId": "lua",
    "fileTypes": [
      "lua",
      "rockspec"
    ],
    "roots": [
      ".luarc.json",
      ".luacheckrc",
      ".stylua.toml",
      "selene.toml",
      ".git"
    ],
    "servers": [
      {
        "name": "lua-language-server"
      }
    ]
  },
  {
    "name": "teal",
    "languageId": "teal",
    "fileTypes": [
      "tl"
    ],
    "roots": [
      "tlconfig.lua"
    ],
    "servers": [
      {
        "name": "teal-language-server"
      }
    ]
  },
  {
    "name": "svelte",
    "languageId": "svelte",
    "fileTypes": [
      "svelte"
    ],
    "servers": [
      {
        "name": "svelteserver"
      }
    ]
  },
  {
    "name": "vue",
    "languageId": "vue",
    "fileTypes": [
      "vue"
    ],
    "roots": [
      "package.json"
    ],
    "servers": [
      {
        "name": "vuels"
      }
    ]
  },
  {
    "name": "yaml",
    "languageId": "yaml",
    "fileTypes": [
      "yml",
      "yaml",
      "sublime-syntax",
      "bu"
    ],
    "servers": [
      {
        "name": "yaml-language-server"
      },
      {
        "name": "ansible-language-server"
      }
    ]
  },
  {
    "name": "haskell",
    "languageId": "haskell",
    "fileTypes": [
      "hs",
      "hs-boot",
      "hsc"
    ],
    "roots": [
      "Setup.hs",
      "stack.yaml",
      "cabal.project",
      "hie.yaml"
    ],
    "servers": [
      {
        "name": "haskell-language-server"
      }
    ]
  },
  {
    "name": "haskell-literate",
    "languageId": "haskell-literate",
    "fileTypes": [
      "lhs"
    ],
    "roots": [
      "Setup.hs",
      "stack.yaml",
      "cabal.project",
      "hie.yaml"
    ],
    "servers": [
      {
        "name": "haskell-language-server"
      }
    ]
  },
  {
    "name": "purescript",
    "languageId": "purescript",
    "fileTypes": [
      "purs"
    ],
    "roots": [
      "spago.yaml",
      "spago.dhall",
      "bower.json"
    ],
    "servers": [
      {
        "name": "purescript-language-server"
      }
    ]
  },
  {
    "name": "zig",
    "languageId": "zig",
    "fileTypes": [
      "zig",
      "zon"
    ],
    "roots": [
      "build.zig"
    ],
    "servers": [
      {
        "name": "zls"
      }
    ]
  },
  {
    "name": "prolog",
    "languageId": "prolog",
    "fileTypes": [
      "pl",
      "prolog"
    ],
    "servers": [
      {
        "name": "swipl"
      }
    ]
  },
  {
    "name": "cmake",
    "languageId": "cmake",
    "fileTypes": [
      "cmake"
    ],
    "servers": [
      {
        "name": "neocmakelsp"
      },
      {
        "name": "cmake-language-server"
      }
    ]
  },
  {
    "name": "glsl",
    "languageId": "glsl",
    "fileTypes": [
      "glsl",
      "vert",
      "tesc",
      "tese",
      "geom",
      "frag",
      "comp"
    ],
    "servers": [
      {
        "name": "glsl_analyzer"
      },
      {
        "name": "glsld"
      }
    ]
  },
  {
    "name": "perl",
    "languageId": "perl",
    "fileTypes": [
      "pl",
      "pm",
      "t",
      "psgi",
      "raku",
      "rakumod",
      "rakutest",
      "rakudoc",
      "nqp",
      "p6",
      "pl6",
      "pm6"
    ],
    "servers": [
      {
        "name": "perlnavigator"
      }
    ]
  },
  {
    "name": "racket",
    "languageId": "racket",
    "fileTypes": [
      "rkt",
      "rktd",
      "rktl",
      "scrbl",
      "zuo"
    ],
    "servers": [
      {
        "name": "racket"
      }
    ]
  },
  {
    "name": "common-lisp",
    "languageId": "common-lisp",
    "fileTypes": [
      "lisp",
      "asd",
      "cl",
      "l",
      "lsp",
      "ny",
      "podsl",
      "ros",
      "sexp"
    ],
    "servers": [
      {
        "name": "cl-lsp"
      }
    ]
  },
  {
    "name": "wgsl",
    "languageId": "wgsl",
    "fileTypes": [
      "wgsl"
    ],
    "servers": [
      {
        "name": "wgsl-analyzer"
      }
    ]
  },
  {
    "name": "markdown",
    "languageId": "markdown",
    "fileTypes": [
      "md",
      "livemd",
      "markdown",
      "mdx",
      "mkd",
      "mkdn",
      "mdwn",
      "mdown",
      "markdn",
      "mdtxt",
      "mdtext",
      "workbook"
    ],
    "roots": [
      ".marksman.toml"
    ],
    "servers": [
      {
        "name": "marksman"
      },
      {
        "name": "markdown-oxide"
      },
      {
        "name": "rumdl"
      }
    ]
  },
  {
    "name": "dart",
    "languageId": "dart",
    "fileTypes": [
      "dart"
    ],
    "roots": [
      "pubspec.yaml"
    ],
    "servers": [
      {
        "name": "dart"
      }
    ]
  },
  {
    "name": "scala",
    "languageId": "scala",
    "fileTypes": [
      "scala",
      "sbt",
      "sc"
    ],
    "roots": [
      "build.sbt",
      "build.sc",
      "build.gradle",
      "build.gradle.kts",
      "pom.xml",
      ".scala-build"
    ],
    "servers": [
      {
        "name": "metals"
      }
    ]
  },
  {
    "name": "dockerfile",
    "languageId": "dockerfile",
    "fileTypes": [
      "Dockerfile",
      "dockerfile",
      "Containerfile",
      "containerfile"
    ],
    "roots": [
      "Dockerfile",
      "Containerfile"
    ],
    "servers": [
      {
        "name": "docker-langserver"
      },
      {
        "name": "docker-language-server"
      }
    ]
  },
  {
    "name": "graphql",
    "languageId": "graphql",
    "fileTypes": [
      "gql",
      "graphql",
      "graphqls"
    ],
    "servers": [
      {
        "name": "graphql-language-service"
      }
    ]
  },
  {
    "name": "elm",
    "languageId": "elm",
    "fileTypes": [
      "elm"
    ],
    "roots": [
      "elm.json"
    ],
    "servers": [
      {
        "name": "elm-language-server"
      }
    ]
  },
  {
    "name": "rescript",
    "languageId": "rescript",
    "fileTypes": [
      "res"
    ],
    "roots": [
      "bsconfig.json"
    ],
    "servers": [
      {
        "name": "rescript-language-server"
      }
    ]
  },
  {
    "name": "erlang",
    "languageId": "erlang",
    "fileTypes": [
      "erl",
      "hrl",
      "app"
    ],
    "roots": [
      "rebar.config"
    ],
    "servers": [
      {
        "name": "erlang-ls",
        "except": [
          "document-symbols",
          "workspace-symbols"
        ]
      },
      {
        "name": "elp",
        "except": [
          "document-symbols",
          "workspace-symbols"
        ]
      }
    ]
  },
  {
    "name": "kotlin",
    "languageId": "kotlin",
    "fileTypes": [
      "kt",
      "kts"
    ],
    "roots": [
      "settings.gradle",
      "settings.gradle.kts"
    ],
    "servers": [
      {
        "name": "kotlin-language-server"
      }
    ]
  },
  {
    "name": "hcl",
    "languageId": "terraform",
    "fileTypes": [
      "hcl",
      "tf",
      "nomad"
    ],
    "servers": [
      {
        "name": "terraform-ls"
      }
    ]
  },
  {
    "name": "tfvars",
    "languageId": "terraform-vars",
    "fileTypes": [
      "tfvars"
    ],
    "servers": [
      {
        "name": "terraform-ls"
      }
    ]
  },
  {
    "name": "solidity",
    "languageId": "solidity",
    "fileTypes": [
      "sol"
    ],
    "servers": [
      {
        "name": "solc"
      }
    ]
  },
  {
    "name": "gleam",
    "languageId": "gleam",
    "fileTypes": [
      "gleam"
    ],
    "roots": [
      "gleam.toml"
    ],
    "servers": [
      {
        "name": "gleam"
      }
    ]
  },
  {
    "name": "ron",
    "languageId": "ron",
    "fileTypes": [
      "ron"
    ],
    "roots": [
      "Cargo.toml"
    ],
    "servers": [
      {
        "name": "ron-lsp"
      }
    ]
  },
  {
    "name": "robot",
    "languageId": "robot",
    "fileTypes": [
      "robot",
      "resource"
    ],
    "servers": [
      {
        "name": "robotcode"
      },
      {
        "name": "robotframework_ls"
      }
    ]
  },
  {
    "name": "r",
    "languageId": "r",
    "fileTypes": [
      "r",
      "R"
    ],
    "servers": [
      {
        "name": "r"
      }
    ]
  },
  {
    "name": "rmarkdown",
    "languageId": "rmd",
    "fileTypes": [
      "rmd",
      "Rmd"
    ],
    "servers": [
      {
        "name": "r"
      }
    ]
  },
  {
    "name": "swift",
    "languageId": "swift",
    "fileTypes": [
      "swift",
      "swiftinterface"
    ],
    "roots": [
      "Package.swift"
    ],
    "servers": [
      {
        "name": "sourcekit-lsp"
      }
    ]
  },
  {
    "name": "heex",
    "languageId": "heex",
    "fileTypes": [
      "heex"
    ],
    "roots": [
      "mix.exs",
      "mix.lock"
    ],
    "servers": [
      {
        "name": "elixir-ls"
      },
      {
        "name": "expert"
      }
    ]
  },
  {
    "name": "nu",
    "languageId": "nu",
    "fileTypes": [
      "nu",
      "nuon"
    ],
    "servers": [
      {
        "name": "nu-lsp"
      },
      {
        "name": "nu-lint"
      }
    ]
  },
  {
    "name": "vala",
    "languageId": "vala",
    "fileTypes": [
      "vala",
      "vapi"
    ],
    "servers": [
      {
        "name": "vala-language-server"
      }
    ]
  },
  {
    "name": "hare",
    "languageId": "hare",
    "fileTypes": [
      "ha"
    ],
    "servers": [
      {
        "name": "hare-lsp",
        "except": [
          "code-action",
          "completion",
          "format",
          "inlay-hints",
          "rename-symbol",
          "workspace-symbols"
        ]
      }
    ]
  },
  {
    "name": "devicetree",
    "languageId": "devicetree",
    "fileTypes": [
      "dts",
      "dtsi"
    ],
    "servers": [
      {
        "name": "dts-lsp"
      }
    ]
  },
  {
    "name": "cairo",
    "languageId": "cairo",
    "fileTypes": [
      "cairo"
    ],
    "servers": [
      {
        "name": "cairo-language-server"
      }
    ]
  },
  {
    "name": "odin",
    "languageId": "odin",
    "fileTypes": [
      "odin"
    ],
    "roots": [
      "ols.json",
      "main.odin"
    ],
    "servers": [
      {
        "name": "ols"
      }
    ]
  },
  {
    "name": "v",
    "languageId": "v",
    "fileTypes": [
      "v",
      "vv",
      "vsh"
    ],
    "roots": [
      "v.mod"
    ],
    "servers": [
      {
        "name": "vlang-language-server"
      }
    ]
  },
  {
    "name": "verilog",
    "languageId": "verilog",
    "fileTypes": [
      "v",
      "vh"
    ],
    "servers": [
      {
        "name": "verible-verilog-ls"
      }
    ]
  },
  {
    "name": "systemverilog",
    "languageId": "systemverilog",
    "fileTypes": [
      "sv",
      "svh"
    ],
    "servers": [
      {
        "name": "svlangserver"
      },
      {
        "name": "verible-verilog-ls"
      }
    ]
  },
  {
    "name": "openscad",
    "languageId": "openscad",
    "fileTypes": [
      "scad"
    ],
    "servers": [
      {
        "name": "openscad-lsp"
      }
    ]
  },
  {
    "name": "prisma",
    "languageId": "prisma",
    "fileTypes": [
      "prisma"
    ],
    "roots": [
      "package.json"
    ],
    "servers": [
      {
        "name": "prisma-language-server"
      }
    ]
  },
  {
    "name": "clojure",
    "languageId": "clojure",
    "fileTypes": [
      "clj",
      "cljs",
      "cljc",
      "clje",
      "cljr",
      "cljx",
      "edn",
      "boot"
    ],
    "roots": [
      "project.clj",
      "build.boot",
      "deps.edn",
      "shadow-cljs.edn"
    ],
    "servers": [
      {
        "name": "clojure-lsp"
      }
    ]
  },
  {
    "name": "starlark",
    "languageId": "starlark",
    "fileTypes": [
      "bzl",
      "bazel",
      "star"
    ],
    "servers": [
      {
        "name": "starpls"
      }
    ]
  },
  {
    "name": "elvish",
    "languageId": "elvish",
    "fileTypes": [
      "elv"
    ],
    "servers": [
      {
        "name": "elvish"
      }
    ]
  },
  {
    "name": "idris",
    "languageId": "idris",
    "fileTypes": [
      "idr"
    ],
    "servers": [
      {
        "name": "idris2-lsp"
      }
    ]
  },
  {
    "name": "fortran",
    "languageId": "fortran",
    "fileTypes": [
      "f",
      "for",
      "f90",
      "f95",
      "f03",
      "F",
      "F90",
      "F95",
      "F03"
    ],
    "roots": [
      "fpm.toml"
    ],
    "servers": [
      {
        "name": "fortls"
      }
    ]
  },
  {
    "name": "dot",
    "languageId": "dot",
    "fileTypes": [
      "dot"
    ],
    "servers": [
      {
        "name": "dot-language-server"
      }
    ]
  },
  {
    "name": "cue",
    "languageId": "cue",
    "fileTypes": [
      "cue"
    ],
    "roots": [
      "cue.mod"
    ],
    "servers": [
      {
        "name": "cuelsp"
      }
    ]
  },
  {
    "name": "slang",
    "languageId": "slang",
    "fileTypes": [
      "slang"
    ],
    "servers": [
      {
        "name": "slangd"
      }
    ]
  },
  {
    "name": "slint",
    "languageId": "slint",
    "fileTypes": [
      "slint"
    ],
    "servers": [
      {
        "name": "slint-lsp"
      }
    ]
  },
  {
    "name": "pascal",
    "languageId": "pascal",
    "fileTypes": [
      "pas",
      "pp",
      "inc",
      "lpr",
      "lfm"
    ],
    "servers": [
      {
        "name": "pasls"
      }
    ]
  },
  {
    "name": "jsonnet",
    "languageId": "jsonnet",
    "fileTypes": [
      "libsonnet",
      "jsonnet"
    ],
    "roots": [
      "jsonnetfile.json"
    ],
    "servers": [
      {
        "name": "jsonnet-language-server"
      }
    ]
  },
  {
    "name": "ada",
    "languageId": "ada",
    "fileTypes": [
      "adb",
      "ads"
    ],
    "roots": [
      "alire.toml"
    ],
    "servers": [
      {
        "name": "ada-language-server"
      }
    ]
  },
  {
    "name": "astro",
    "languageId": "astro",
    "fileTypes": [
      "astro"
    ],
    "servers": [
      {
        "name": "astro-ls"
      }
    ]
  },
  {
    "name": "bass",
    "languageId": "bass",
    "fileTypes": [
      "bass"
    ],
    "servers": [
      {
        "name": "bass"
      }
    ]
  },
  {
    "name": "wat",
    "languageId": "wat",
    "fileTypes": [
      "wat"
    ],
    "servers": [
      {
        "name": "wasm-language-tools"
      }
    ]
  },
  {
    "name": "d",
    "languageId": "d",
    "fileTypes": [
      "d",
      "dd"
    ],
    "servers": [
      {
        "name": "serve-d"
      }
    ]
  },
  {
    "name": "systemd",
    "languageId": "systemd",
    "fileTypes": [
      "service",
      "automount",
      "device",
      "mount",
      "nspawn",
      "path",
      "scope",
      "slice",
      "socket",
      "swap",
      "target",
      "timer",
      "container",
      "kube",
      "network",
      "volume",
      "build",
      "image"
    ],
    "servers": [
      {
        "name": "systemd-lsp"
      }
    ]
  },
  {
    "name": "bicep",
    "languageId": "bicep",
    "fileTypes": [
      "bicep",
      "bicepparam"
    ],
    "servers": [
      {
        "name": "bicep-langserver"
      }
    ]
  },
  {
    "name": "qml",
    "languageId": "qml",
    "fileTypes": [
      "qml"
    ],
    "servers": [
      {
        "name": "qmlls"
      }
    ]
  },
  {
    "name": "ponylang",
    "languageId": "ponylang",
    "fileTypes": [
      "pony"
    ],
    "roots": [
      "corral.json",
      "lock.json"
    ],
    "servers": [
      {
        "name": "pony-lsp"
      }
    ]
  },
  {
    "name": "dhall",
    "languageId": "dhall",
    "fileTypes": [
      "dhall"
    ],
    "servers": [
      {
        "name": "dhall-lsp-server"
      }
    ]
  },
  {
    "name": "nasm",
    "languageId": "nasm",
    "fileTypes": [
      "asm",
      "S",
      "nasm"
    ],
    "servers": [
      {
        "name": "asm-lsp"
      }
    ]
  },
  {
    "name": "gas",
    "languageId": "gas",
    "fileTypes": [
      "s"
    ],
    "servers": [
      {
        "name": "asm-lsp"
      }
    ]
  },
  {
    "name": "smithy",
    "languageId": "smithy",
    "fileTypes": [
      "smithy"
    ],
    "roots": [
      "smithy-build.json"
    ],
    "servers": [
      {
        "name": "cs"
      }
    ]
  },
  {
    "name": "hdl",
    "languageId": "hdl",
    "fileTypes": [
      "hdl"
    ],
    "servers": [
      {
        "name": "hdls"
      }
    ]
  },
  {
    "name": "vhdl",
    "languageId": "vhdl",
    "fileTypes": [
      "vhd",
      "vhdl"
    ],
    "servers": [
      {
        "name": "vhdl_ls"
      }
    ]
  },
  {
    "name": "rego",
    "languageId": "rego",
    "fileTypes": [
      "rego"
    ],
    "servers": [
      {
        "name": "regols"
      }
    ]
  },
  {
    "name": "nim",
    "languageId": "nim",
    "fileTypes": [
      "nim",
      "nims",
      "nimble"
    ],
    "servers": [
      {
        "name": "nimlangserver"
      }
    ]
  },
  {
    "name": "cabal",
    "languageId": "cabal",
    "fileTypes": [
      "cabal"
    ],
    "roots": [
      "cabal.project",
      "Setup.hs"
    ],
    "servers": [
      {
        "name": "haskell-language-server"
      }
    ]
  },
  {
    "name": "markdoc",
    "languageId": "markdoc",
    "fileTypes": [
      "mdoc"
    ],
    "servers": [
      {
        "name": "markdoc-ls"
      }
    ]
  },
  {
    "name": "opencl",
    "languageId": "opencl",
    "fileTypes": [
      "cl"
    ],
    "servers": [
      {
        "name": "clangd"
      }
    ]
  },
  {
    "name": "just",
    "languageId": "just",
    "fileTypes": [
      "just"
    ],
    "servers": [
      {
        "name": "just-lsp"
      }
    ]
  },
  {
    "name": "blueprint",
    "languageId": "blueprint",
    "fileTypes": [
      "blp"
    ],
    "servers": [
      {
        "name": "blueprint-compiler"
      }
    ]
  },
  {
    "name": "forth",
    "languageId": "forth",
    "fileTypes": [
      "fs",
      "forth",
      "fth",
      "4th"
    ],
    "servers": [
      {
        "name": "forth-lsp"
      }
    ]
  },
  {
    "name": "fsharp",
    "languageId": "fsharp",
    "fileTypes": [
      "fs",
      "fsx",
      "fsi",
      "fsscript"
    ],
    "roots": [
      "*.slnx",
      "*.sln",
      "*.fsproj"
    ],
    "servers": [
      {
        "name": "fsharp-ls"
      }
    ]
  },
  {
    "name": "typst",
    "languageId": "typst",
    "fileTypes": [
      "typst",
      "typ"
    ],
    "servers": [
      {
        "name": "tinymist"
      }
    ]
  },
  {
    "name": "jq",
    "languageId": "jq",
    "fileTypes": [
      "jq"
    ],
    "servers": [
      {
        "name": "jq-lsp"
      }
    ]
  },
  {
    "name": "templ",
    "languageId": "templ",
    "fileTypes": [
      "templ"
    ],
    "roots": [
      "go.work",
      "go.mod"
    ],
    "servers": [
      {
        "name": "templ"
      }
    ]
  },
  {
    "name": "bitbake",
    "languageId": "bitbake",
    "fileTypes": [
      "bb",
      "bbappend",
      "bbclass"
    ],
    "servers": [
      {
        "name": "bitbake-language-server"
      }
    ]
  },
  {
    "name": "koka",
    "languageId": "koka",
    "fileTypes": [
      "kk"
    ],
    "servers": [
      {
        "name": "koka"
      }
    ]
  },
  {
    "name": "pkl",
    "languageId": "pkl",
    "fileTypes": [
      "pkl",
      "pcf"
    ],
    "servers": [
      {
        "name": "pkl-lsp"
      }
    ]
  },
  {
    "name": "hy",
    "languageId": "hy",
    "fileTypes": [
      "hy"
    ],
    "roots": [
      "pyproject.toml"
    ],
    "servers": [
      {
        "name": "hyuga"
      }
    ]
  },
  {
    "name": "pest",
    "languageId": "pest",
    "fileTypes": [
      "pest"
    ],
    "servers": [
      {
        "name": "pest-language-server"
      }
    ]
  },
  {
    "name": "gjs",
    "languageId": "gjs",
    "fileTypes": [
      "gjs"
    ],
    "roots": [
      "package.json",
      "ember-cli-build.js"
    ],
    "servers": [
      {
        "name": "typescript-language-server",
        "except": [
          "format",
          "diagnostics"
        ]
      },
      {
        "name": "vscode-eslint-language-server"
      },
      {
        "name": "ember-language-server"
      }
    ]
  },
  {
    "name": "gts",
    "languageId": "gts",
    "fileTypes": [
      "gts"
    ],
    "roots": [
      "package.json",
      "ember-cli-build.js"
    ],
    "servers": [
      {
        "name": "typescript-language-server",
        "except": [
          "format",
          "diagnostics"
        ]
      },
      {
        "name": "vscode-eslint-language-server"
      },
      {
        "name": "ember-language-server"
      }
    ]
  },
  {
    "name": "circom",
    "languageId": "circom",
    "fileTypes": [
      "circom"
    ],
    "roots": [
      "package.json"
    ],
    "servers": [
      {
        "name": "circom-lsp"
      }
    ]
  },
  {
    "name": "snakemake",
    "languageId": "snakemake",
    "fileTypes": [
      "smk"
    ],
    "roots": [
      "Snakefile",
      "config.yaml",
      "environment.yaml",
      "workflow/"
    ],
    "servers": [
      {
        "name": "pylsp"
      }
    ]
  },
  {
    "name": "quint",
    "languageId": "quint",
    "fileTypes": [
      "qnt"
    ],
    "servers": [
      {
        "name": "quint-language-server"
      }
    ]
  },
  {
    "name": "spade",
    "languageId": "spade",
    "fileTypes": [
      "spade"
    ],
    "roots": [
      "swim.toml"
    ],
    "servers": [
      {
        "name": "spade-language-server"
      }
    ]
  },
  {
    "name": "amber",
    "languageId": "amber",
    "fileTypes": [
      "ab"
    ],
    "servers": [
      {
        "name": "amber-lsp"
      }
    ]
  },
  {
    "name": "koto",
    "languageId": "koto",
    "fileTypes": [
      "koto"
    ],
    "servers": [
      {
        "name": "koto-ls"
      }
    ]
  },
  {
    "name": "gpr",
    "languageId": "gpr",
    "fileTypes": [
      "gpr"
    ],
    "roots": [
      "alire.toml"
    ],
    "servers": [
      {
        "name": "ada-gpr-language-server"
      }
    ]
  },
  {
    "name": "codeql",
    "languageId": "codeql",
    "fileTypes": [
      "ql",
      "qll"
    ],
    "servers": [
      {
        "name": "codeql"
      }
    ]
  },
  {
    "name": "yara",
    "languageId": "yara",
    "fileTypes": [
      "yara",
      "yar"
    ],
    "servers": [
      {
        "name": "yls"
      }
    ]
  },
  {
    "name": "sourcepawn",
    "languageId": "sourcepawn",
    "fileTypes": [
      "sp",
      "inc"
    ],
    "servers": [
      {
        "name": "sourcepawn-studio"
      }
    ]
  },
  {
    "name": "clarity",
    "languageId": "clarity",
    "fileTypes": [
      "clar"
    ],
    "roots": [
      "Clarinet.toml"
    ],
    "servers": [
      {
        "name": "clarinet"
      }
    ]
  },
  {
    "name": "luau",
    "languageId": "luau",
    "fileTypes": [
      "luau"
    ],
    "roots": [
      "aftman.toml",
      "default.project.json",
      "wally.toml",
      "rokit.toml",
      "selene.toml",
      ".darklua.json",
      "foreman.toml",
      ".luaurc"
    ],
    "servers": [
      {
        "name": "luau"
      }
    ]
  },
  {
    "name": "wikitext",
    "languageId": "wikitext",
    "fileTypes": [
      "wikimedia",
      "mediawiki",
      "wikitext"
    ],
    "servers": [
      {
        "name": "wikitext-lsp"
      }
    ]
  },
  {
    "name": "kcl",
    "languageId": "kcl",
    "fileTypes": [
      "kcl"
    ],
    "servers": [
      {
        "name": "kcl-lsp"
      }
    ]
  },
  {
    "name": "haxe",
    "languageId": "haxe",
    "fileTypes": [
      "hx"
    ],
    "roots": [
      "haxelib.json",
      "project.xml",
      "build.hxml"
    ],
    "servers": [
      {
        "name": "haxe-language-server"
      }
    ]
  },
  {
    "name": "ripple",
    "languageId": "ripple",
    "fileTypes": [
      "ripple"
    ],
    "roots": [
      "package.json"
    ],
    "servers": [
      {
        "name": "ripple-lsp"
      }
    ]
  },
  {
    "name": "styx",
    "languageId": "styx",
    "fileTypes": [
      "styx"
    ],
    "servers": [
      {
        "name": "styx"
      }
    ]
  },
  {
    "name": "tilt",
    "languageId": "tilt",
    "fileTypes": [
      "tiltfile"
    ],
    "servers": [
      {
        "name": "tilt"
      }
    ]
  },
  {
    "name": "ebnf",
    "languageId": "ebnf",
    "fileTypes": [
      "ebnf"
    ],
    "servers": [
      {
        "name": "ebnfer"
      }
    ]
  },
  {
    "name": "drools",
    "languageId": "drools",
    "fileTypes": [
      "drl"
    ],
    "roots": [
      "pom.xml",
      ".git"
    ],
    "servers": [
      {
        "name": "drools-lsp"
      }
    ]
  }
];
