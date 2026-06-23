// GENERATED FILE — do not edit.
// Source: helix-editor/helix languages.toml.
// Regenerate: pnpm --filter @the-editor/syntax codegen [path-to-languages.toml]

import type { SyntaxConfiguration } from "../syntax.js";

export const generatedLanguages: SyntaxConfiguration = {
  "language": [
    {
      "name": "rust",
      "scope": "source.rust",
      "fileTypes": [
        "rs"
      ],
      "shebangs": [
        "rust-script",
        "cargo"
      ],
      "commentTokens": [
        "//",
        "///",
        "//!"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        },
        {
          "start": "/**",
          "end": "*/"
        },
        {
          "start": "/*!",
          "end": "*/"
        }
      ],
      "injectionRegex": "rs|rust",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "sway",
      "scope": "source.sway",
      "fileTypes": [
        "sw"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "sway",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "toml",
      "scope": "source.toml",
      "fileTypes": [
        "toml",
        {
          "glob": "pdm.lock"
        },
        {
          "glob": "poetry.lock"
        },
        {
          "glob": "Cargo.lock"
        },
        {
          "glob": "uv.lock"
        },
        {
          "glob": "containers.conf"
        },
        {
          "glob": "containers.conf.d/*.conf"
        },
        {
          "glob": "containers.conf.modules/*.conf"
        },
        {
          "glob": "mounts.conf"
        },
        {
          "glob": "policy.conf"
        },
        {
          "glob": "registries.conf"
        },
        {
          "glob": "storage.conf"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "toml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "awk",
      "scope": "source.awk",
      "fileTypes": [
        "awk",
        "gawk",
        "nawk",
        "mawk"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "awk",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "protobuf",
      "scope": "source.proto",
      "fileTypes": [
        "proto"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "proto",
      "grammar": "proto",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "textproto",
      "scope": "source.textproto",
      "fileTypes": [
        "txtpb",
        "textpb",
        "textproto"
      ],
      "commentTokens": [
        "#"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "eiffel",
      "scope": "source.eiffel",
      "fileTypes": [
        "e"
      ],
      "commentTokens": [
        "--"
      ]
    },
    {
      "name": "elixir",
      "scope": "source.elixir",
      "fileTypes": [
        "ex",
        "exs",
        {
          "glob": "mix.lock"
        }
      ],
      "shebangs": [
        "elixir"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "(elixir|ex)",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "fennel",
      "scope": "source.fennel",
      "fileTypes": [
        "fnl",
        "fnlm"
      ],
      "shebangs": [
        "fennel"
      ],
      "commentTokens": [
        ";"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "fish",
      "scope": "source.fish",
      "fileTypes": [
        "fish"
      ],
      "shebangs": [
        "fish"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "fish",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "flatbuffers",
      "scope": "source.flatbuffers",
      "fileTypes": [
        "fbs"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "(flatbuffers?|fbs)",
      "indent": {
        "tabWidth": 2,
        "unit": " "
      }
    },
    {
      "name": "mint",
      "scope": "source.mint",
      "fileTypes": [
        "mint"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "mint",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "mojo",
      "scope": "source.mojo",
      "fileTypes": [
        "mojo",
        "🔥"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "mojo",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "janet",
      "scope": "source.janet",
      "fileTypes": [
        "cgen",
        "janet",
        "jdn"
      ],
      "shebangs": [
        "janet"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "janet",
      "grammar": "janet-simple",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "json",
      "scope": "source.json",
      "fileTypes": [
        "json",
        "arb",
        "ipynb",
        "geojson",
        "gltf",
        "webmanifest",
        {
          "glob": "flake.lock"
        },
        {
          "glob": ".babelrc"
        },
        {
          "glob": ".bowerrc"
        },
        {
          "glob": ".jscrc"
        },
        "js.map",
        "ts.map",
        "css.map",
        {
          "glob": ".jslintrc"
        },
        "jsonl",
        {
          "glob": ".vuerc"
        },
        {
          "glob": "composer.lock"
        },
        {
          "glob": ".watchmanconfig"
        },
        "avsc",
        "ldtk",
        "ldtkl",
        {
          "glob": ".swift-format"
        },
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
        "code-workspace",
        {
          "glob": ".pytest_cache/v/cache/*"
        },
        {
          "glob": "devenv.lock"
        },
        {
          "glob": "devbox.lock"
        },
        {
          "glob": "manifest.lock"
        },
        {
          "glob": "deno.lock"
        }
      ],
      "injectionRegex": "json",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "jsonc",
      "scope": "source.json",
      "fileTypes": [
        "jsonc",
        {
          "glob": "{t,j}sconfig.json"
        },
        {
          "glob": "bun.lock"
        },
        {
          "glob": ".devcontainer.json"
        },
        {
          "glob": "devcontainer.json"
        },
        {
          "glob": ".vscode/*.json"
        }
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "jsonc",
      "grammar": "json",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "json-ld",
      "scope": "source.json-ld",
      "fileTypes": [
        "jsonld"
      ],
      "injectionRegex": "json-ld",
      "grammar": "json",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "json5",
      "scope": "source.json5",
      "fileTypes": [
        "json5"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "json5",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "c",
      "scope": "source.c",
      "fileTypes": [
        "c"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "c",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "cpp",
      "scope": "source.cpp",
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
        "inl",
        {
          "glob": ".hpp.in"
        },
        {
          "glob": ".h.in"
        }
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "cpp",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "crystal",
      "scope": "source.cr",
      "fileTypes": [
        "cr"
      ],
      "commentTokens": [
        "#"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "c-sharp",
      "scope": "source.csharp",
      "fileTypes": [
        "cs",
        "csx",
        "cake"
      ],
      "commentTokens": [
        "//",
        "///"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "c-?sharp",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "c3",
      "scope": "source.c3",
      "fileTypes": [
        "c3",
        "c3i",
        "c3t"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        },
        {
          "start": "<*",
          "end": "*>"
        }
      ],
      "injectionRegex": "c3",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "cel",
      "scope": "source.cel",
      "fileTypes": [
        "cel"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "cel",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "spicedb",
      "scope": "source.zed",
      "fileTypes": [
        "zed"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "spicedb",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "go",
      "scope": "source.go",
      "fileTypes": [
        "go"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "go",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "gomod",
      "scope": "source.gomod",
      "fileTypes": [
        {
          "glob": "go.mod"
        }
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "gomod",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "gotmpl",
      "scope": "source.gotmpl",
      "fileTypes": [
        "gotmpl"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "gotmpl",
      "indent": {
        "tabWidth": 2,
        "unit": " "
      }
    },
    {
      "name": "gowork",
      "scope": "source.gowork",
      "fileTypes": [
        {
          "glob": "go.work"
        }
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "gowork",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "go-format-string",
      "scope": "source.go-format-string",
      "fileTypes": [],
      "injectionRegex": "go-format-string"
    },
    {
      "name": "javascript",
      "scope": "source.js",
      "fileTypes": [
        "js",
        "mjs",
        "cjs",
        "rules",
        "es6",
        "pac",
        "gs",
        {
          "glob": ".node_repl_history"
        },
        {
          "glob": "jakefile"
        }
      ],
      "shebangs": [
        "node"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "(js|javascript)",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "jsx",
      "scope": "source.jsx",
      "fileTypes": [
        "jsx"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "jsx",
      "grammar": "javascript",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "typescript",
      "scope": "source.ts",
      "fileTypes": [
        "ts",
        "mts",
        "cts"
      ],
      "shebangs": [
        "deno",
        "bun",
        "ts-node"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "(ts|typescript)",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "typespec",
      "scope": "source.typespec",
      "fileTypes": [
        "tsp"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "(tsp|typespec)",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "tsx",
      "scope": "source.tsx",
      "fileTypes": [
        "tsx"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "(tsx)",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "css",
      "scope": "source.css",
      "fileTypes": [
        "css"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "css",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "scss",
      "scope": "source.scss",
      "fileTypes": [
        "scss"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "scss",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "less",
      "scope": "source.less",
      "fileTypes": [
        "less"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "less",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "html",
      "scope": "text.html.basic",
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
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "html",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "htmldjango",
      "scope": "source.htmldjango",
      "fileTypes": [],
      "injectionRegex": "htmldjango"
    },
    {
      "name": "python",
      "scope": "source.python",
      "fileTypes": [
        "py",
        "pyi",
        "py3",
        "pyw",
        "ptl",
        "rpy",
        "cpy",
        "ipy",
        "pyt",
        {
          "glob": ".python_history"
        },
        {
          "glob": ".pythonstartup"
        },
        {
          "glob": ".pythonrc"
        },
        {
          "glob": "*SConstruct"
        },
        {
          "glob": "*SConscript"
        },
        {
          "glob": "*sconstruct"
        }
      ],
      "shebangs": [
        "python",
        "uv"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "py(thon)?",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "nickel",
      "scope": "source.nickel",
      "fileTypes": [
        "ncl"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "nickel",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "nix",
      "scope": "source.nix",
      "fileTypes": [
        "nix"
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "nix",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "ruby",
      "scope": "source.ruby",
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
        "rbs",
        {
          "glob": "rakefile"
        },
        {
          "glob": "gemfile"
        },
        {
          "glob": "Rakefile"
        },
        {
          "glob": "Gemfile"
        },
        {
          "glob": "Podfile"
        },
        {
          "glob": "Vagrantfile"
        },
        {
          "glob": "Brewfile"
        },
        {
          "glob": "*.Brewfile"
        },
        {
          "glob": "Guardfile"
        },
        {
          "glob": "Capfile"
        },
        {
          "glob": "Cheffile"
        },
        {
          "glob": "Hobofile"
        },
        {
          "glob": "Appraisals"
        },
        {
          "glob": "Rantfile"
        },
        {
          "glob": "Berksfile"
        },
        {
          "glob": "Berksfile.lock"
        },
        {
          "glob": "Thorfile"
        },
        {
          "glob": "Puppetfile"
        },
        {
          "glob": "Fastfile"
        },
        {
          "glob": "Appfile"
        },
        {
          "glob": "Deliverfile"
        },
        {
          "glob": "Matchfile"
        },
        {
          "glob": "Scanfile"
        },
        {
          "glob": "Snapfile"
        },
        {
          "glob": "Gymfile"
        },
        {
          "glob": ".irbrc"
        }
      ],
      "shebangs": [
        "ruby"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "ruby",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "rshtml",
      "scope": "source.rshtml",
      "fileTypes": [
        {
          "glob": "*.rs.html"
        }
      ],
      "blockCommentTokens": [
        {
          "start": "@*",
          "end": "*@"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "bash",
      "scope": "source.bash",
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
        "zshrc_Apple_Terminal",
        {
          "glob": "i3/config"
        },
        {
          "glob": "sway/config"
        },
        {
          "glob": ".tmux.conf"
        },
        {
          "glob": "tmux.conf"
        },
        {
          "glob": ".sh_history"
        },
        {
          "glob": ".bash_history"
        },
        {
          "glob": ".bash_login"
        },
        {
          "glob": ".bash_logout"
        },
        {
          "glob": ".bash_profile"
        },
        {
          "glob": ".bashrc"
        },
        {
          "glob": ".profile"
        },
        {
          "glob": ".zshenv"
        },
        {
          "glob": ".zlogin"
        },
        {
          "glob": ".zlogout"
        },
        {
          "glob": ".zprofile"
        },
        {
          "glob": ".zshrc"
        },
        {
          "glob": ".zimrc"
        },
        {
          "glob": "APKBUILD"
        },
        {
          "glob": ".bash_aliases"
        },
        {
          "glob": "bash_completion"
        },
        {
          "glob": "bash-completion/completions/*"
        },
        {
          "glob": "bash_completion.d/*"
        },
        {
          "glob": "bin/activate"
        },
        {
          "glob": ".Renviron"
        },
        {
          "glob": ".xprofile"
        },
        {
          "glob": ".xsession"
        },
        {
          "glob": ".xsessionrc"
        },
        {
          "glob": ".yashrc"
        },
        {
          "glob": ".yash_profile"
        },
        {
          "glob": ".hushlogin"
        },
        {
          "glob": ".xinitrc"
        },
        {
          "glob": "xinitrc"
        },
        {
          "glob": ".xserverrc"
        },
        {
          "glob": "xserverrc"
        },
        {
          "glob": "direnvrc"
        }
      ],
      "shebangs": [
        "sh",
        "bash",
        "dash",
        "zsh"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "(shell|bash|zsh|sh)",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "php",
      "scope": "source.php",
      "fileTypes": [
        "php",
        "inc",
        "php4",
        "php5",
        "phtml",
        "ctp"
      ],
      "shebangs": [
        "php"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/**",
          "end": "*/"
        },
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "php",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "php-only",
      "scope": "source.php-only",
      "fileTypes": [],
      "injectionRegex": "php-only",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "blade",
      "scope": "source.blade.php",
      "fileTypes": [
        {
          "glob": "*.blade.php"
        },
        "blade"
      ],
      "injectionRegex": "blade"
    },
    {
      "name": "twig",
      "scope": "source.twig",
      "fileTypes": [
        "twig"
      ],
      "blockCommentTokens": [
        {
          "start": "{#",
          "end": "#}"
        }
      ],
      "injectionRegex": "twig",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "latex",
      "scope": "source.tex",
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
      "commentTokens": [
        "%"
      ],
      "injectionRegex": "tex",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "bibtex",
      "scope": "source.bib",
      "fileTypes": [
        "bib"
      ],
      "commentTokens": [
        "%"
      ],
      "injectionRegex": "bib",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "lean",
      "scope": "source.lean",
      "fileTypes": [
        "lean"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "/-",
          "end": "-/"
        }
      ],
      "injectionRegex": "lean",
      "textWidth": 100,
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      },
      "rulers": [
        101
      ]
    },
    {
      "name": "lpf",
      "scope": "source.lpf",
      "fileTypes": [
        "lpf"
      ],
      "commentTokens": [
        "#"
      ]
    },
    {
      "name": "julia",
      "scope": "source.julia",
      "fileTypes": [
        "jl"
      ],
      "shebangs": [
        "julia"
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "#=",
          "end": "=#"
        }
      ],
      "injectionRegex": "julia",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "java",
      "scope": "source.java",
      "fileTypes": [
        "java",
        "jav",
        "pde"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "java",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "smali",
      "scope": "source.smali",
      "fileTypes": [
        "smali"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "smali",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "ledger",
      "scope": "source.ledger",
      "fileTypes": [
        "ldg",
        "ledger",
        "journal"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "ledger",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "beancount",
      "scope": "source.beancount",
      "fileTypes": [
        "beancount",
        "bean"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "beancount",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "ocaml",
      "scope": "source.ocaml",
      "fileTypes": [
        "ml"
      ],
      "shebangs": [
        "ocaml",
        "ocamlrun",
        "ocamlscript"
      ],
      "blockCommentTokens": [
        {
          "start": "(*",
          "end": "*)"
        }
      ],
      "injectionRegex": "ocaml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "ocaml-interface",
      "scope": "source.ocaml.interface",
      "fileTypes": [
        "mli"
      ],
      "commentTokens": [
        "(**)"
      ],
      "blockCommentTokens": [
        {
          "start": "(*",
          "end": "*)"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "dune",
      "scope": "source.dune",
      "fileTypes": [
        {
          "glob": "dune-project"
        },
        {
          "glob": "dune"
        }
      ],
      "commentTokens": [
        ";"
      ],
      "grammar": "scheme",
      "indent": {
        "tabWidth": 1,
        "unit": " "
      }
    },
    {
      "name": "lua",
      "scope": "source.lua",
      "fileTypes": [
        "lua",
        "rockspec"
      ],
      "shebangs": [
        "lua",
        "luajit"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "--[[",
          "end": "--]]"
        }
      ],
      "injectionRegex": "lua",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "luap",
      "scope": "source.luap",
      "fileTypes": [],
      "injectionRegex": "luap"
    },
    {
      "name": "lua-format-string",
      "scope": "source.lua-format-string",
      "fileTypes": [],
      "injectionRegex": "lua-format-string"
    },
    {
      "name": "teal",
      "scope": "source.tl",
      "fileTypes": [
        "tl"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "--[[",
          "end": "--]]"
        }
      ],
      "injectionRegex": "teal"
    },
    {
      "name": "svelte",
      "scope": "source.svelte",
      "fileTypes": [
        "svelte"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "svelte",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "vue",
      "scope": "source.vue",
      "fileTypes": [
        "vue"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "vue",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "yaml",
      "scope": "source.yaml",
      "fileTypes": [
        "yml",
        "yaml",
        {
          "glob": ".prettierrc"
        },
        {
          "glob": ".clangd"
        },
        {
          "glob": ".clang-format"
        },
        {
          "glob": ".clang-tidy"
        },
        {
          "glob": ".gem/credentials"
        },
        {
          "glob": ".kube/config"
        },
        {
          "glob": ".kube/kuberc"
        },
        {
          "glob": "yarn.lock"
        },
        "sublime-syntax",
        "bu",
        {
          "glob": ".stylelintrc"
        },
        {
          "glob": ".puppeteerrc"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "yml|yaml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "nestedtext",
      "scope": "text.nested",
      "fileTypes": [
        "nt"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "nestedtext",
      "grammar": "yaml",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "haskell",
      "scope": "source.haskell",
      "fileTypes": [
        "hs",
        "hs-boot",
        "hsc"
      ],
      "shebangs": [
        "runhaskell",
        "stack"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "{-",
          "end": "-}"
        }
      ],
      "injectionRegex": "hs|haskell",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "haskell-persistent",
      "scope": "source.persistentmodels",
      "fileTypes": [
        "persistentmodels"
      ],
      "commentTokens": [
        "--"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "haskell-literate",
      "scope": "source.haskell-literate",
      "fileTypes": [
        "lhs"
      ],
      "shebangs": [
        "runhaskell",
        "stack"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "{-",
          "end": "-}"
        }
      ],
      "injectionRegex": "lhs",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "purescript",
      "scope": "source.purescript",
      "fileTypes": [
        "purs"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "{-",
          "end": "-}"
        }
      ],
      "injectionRegex": "purescript",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "zig",
      "scope": "source.zig",
      "fileTypes": [
        "zig",
        "zon"
      ],
      "commentTokens": [
        "//",
        "///",
        "//!"
      ],
      "injectionRegex": "zig",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "picat",
      "scope": "source.picat",
      "fileTypes": [
        "pi",
        "picat"
      ],
      "shebangs": [
        "picat"
      ],
      "commentTokens": [
        "%"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "(pi|picat)",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "prolog",
      "scope": "source.prolog",
      "fileTypes": [
        "pl",
        "prolog"
      ],
      "shebangs": [
        "swipl"
      ],
      "commentTokens": [
        "%"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ]
    },
    {
      "name": "tsq",
      "scope": "source.tsq",
      "fileTypes": [
        {
          "glob": "queries/*.scm"
        },
        {
          "glob": "injections.scm"
        },
        {
          "glob": "highlights.scm"
        },
        {
          "glob": "indents.scm"
        },
        {
          "glob": "textobjects.scm"
        },
        {
          "glob": "locals.scm"
        },
        {
          "glob": "tags.scm"
        }
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "tsq",
      "grammar": "query",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "cmake",
      "scope": "source.cmake",
      "fileTypes": [
        "cmake",
        {
          "glob": "CMakeLists.txt"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "#[[",
          "end": "]]"
        }
      ],
      "injectionRegex": "cmake",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "make",
      "scope": "source.make",
      "fileTypes": [
        {
          "glob": "Makefile"
        },
        {
          "glob": "makefile"
        },
        "make",
        "mk",
        "mak",
        {
          "glob": "GNUmakefile"
        },
        {
          "glob": "OCamlMakefile"
        }
      ],
      "shebangs": [
        "make",
        "gmake"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "(make|makefile|Makefile|mk)",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "glsl",
      "scope": "source.glsl",
      "fileTypes": [
        "glsl",
        "vert",
        "tesc",
        "tese",
        "geom",
        "frag",
        "comp"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "glsl",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "penrose",
      "scope": "source.penrose",
      "fileTypes": [
        "substance",
        "style",
        "domain"
      ],
      "commentTokens": [
        "--"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "perl",
      "scope": "source.perl",
      "fileTypes": [
        {
          "glob": "*.pl"
        },
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
        "pm6",
        {
          "glob": "latexmkrc"
        },
        {
          "glob": ".latexmkrc"
        }
      ],
      "shebangs": [
        "perl"
      ],
      "commentTokens": [
        "#"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "embedded-perl",
      "scope": "text.html.ep",
      "fileTypes": [
        "ep",
        {
          "glob": "*.html.ep"
        }
      ],
      "commentTokens": [
        "%#"
      ],
      "injectionRegex": "embedded-perl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "pod",
      "scope": "source.pod",
      "fileTypes": [
        "pod"
      ],
      "injectionRegex": "pod"
    },
    {
      "name": "racket",
      "scope": "source.racket",
      "fileTypes": [
        "rkt",
        "rktd",
        "rktl",
        "scrbl",
        "zuo"
      ],
      "shebangs": [
        "racket",
        "zuo"
      ],
      "commentTokens": [
        ";"
      ],
      "blockCommentTokens": [
        {
          "start": "#|",
          "end": "|#"
        }
      ],
      "grammar": "scheme",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "common-lisp",
      "scope": "source.commonlisp",
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
      "shebangs": [
        "lisp",
        "sbcl",
        "ccl",
        "clisp",
        "ecl"
      ],
      "commentTokens": [
        ";"
      ],
      "grammar": "commonlisp",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "comment",
      "scope": "scope.comment",
      "fileTypes": [],
      "injectionRegex": "comment"
    },
    {
      "name": "wesl",
      "scope": "source.wesl",
      "fileTypes": [
        "wesl"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "wgsl",
      "scope": "source.wgsl",
      "fileTypes": [
        "wgsl"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "llvm",
      "scope": "source.llvm",
      "fileTypes": [
        "ll"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "llvm",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "llvm-mir",
      "scope": "source.llvm_mir",
      "fileTypes": [],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "mir",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "llvm-mir-yaml",
      "scope": "source.yaml",
      "fileTypes": [
        "mir"
      ],
      "commentTokens": [
        "#"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "tablegen",
      "scope": "source.tablegen",
      "fileTypes": [
        "td"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "tablegen",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "mail",
      "scope": "text.mail",
      "fileTypes": [
        "eml"
      ],
      "injectionRegex": "mail|eml|email"
    },
    {
      "name": "markdown",
      "scope": "source.md",
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
        "workbook",
        {
          "glob": "PULLREQ_EDITMSG"
        }
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "md|markdown",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "markdown-rustdoc",
      "scope": "source.markdown-rustdoc",
      "fileTypes": [],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "markdown-rustdoc",
      "grammar": "markdown",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "markdown.inline",
      "scope": "source.markdown.inline",
      "fileTypes": [],
      "injectionRegex": "markdown\\.inline",
      "grammar": "markdown_inline"
    },
    {
      "name": "djot",
      "scope": "source.djot",
      "fileTypes": [
        "dj",
        "djot"
      ],
      "blockCommentTokens": [
        {
          "start": "{%",
          "end": "%}"
        }
      ],
      "injectionRegex": "dj|djot",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "dart",
      "scope": "source.dart",
      "fileTypes": [
        "dart"
      ],
      "commentTokens": [
        "//",
        "///"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "scala",
      "scope": "source.scala",
      "fileTypes": [
        "scala",
        "sbt",
        "sc"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "dockerfile",
      "scope": "source.dockerfile",
      "fileTypes": [
        "Dockerfile",
        {
          "glob": "Dockerfile"
        },
        {
          "glob": "Dockerfile.*"
        },
        "dockerfile",
        {
          "glob": "dockerfile"
        },
        {
          "glob": "dockerfile.*"
        },
        "Containerfile",
        {
          "glob": "Containerfile"
        },
        {
          "glob": "Containerfile.*"
        },
        "containerfile",
        {
          "glob": "containerfile"
        },
        {
          "glob": "containerfile.*"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "docker|dockerfile",
      "indent": {
        "tabWidth": 4,
        "unit": "  "
      }
    },
    {
      "name": "docker-compose",
      "scope": "source.yaml.docker-compose",
      "fileTypes": [
        {
          "glob": "docker-compose.yaml"
        },
        {
          "glob": "docker-compose.yml"
        },
        {
          "glob": "compose.yaml"
        },
        {
          "glob": "compose.yml"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "grammar": "yaml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "git-commit",
      "scope": "git.commitmsg",
      "fileTypes": [
        {
          "glob": "COMMIT_EDITMSG"
        },
        {
          "glob": "MERGE_MSG"
        },
        {
          "glob": "TAG_EDITMSG"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "grammar": "gitcommit",
      "textWidth": 72,
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      },
      "rulers": [
        51,
        73
      ]
    },
    {
      "name": "git-notes",
      "scope": "git.notesmsg",
      "fileTypes": [
        {
          "glob": "NOTES_EDITMSG"
        },
        {
          "glob": "EDIT_DESCRIPTION"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "grammar": "gitcommit",
      "textWidth": 72,
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      },
      "rulers": [
        73
      ]
    },
    {
      "name": "github-action",
      "scope": "source.yaml.github-action",
      "fileTypes": [
        {
          "glob": ".github/workflows/*.yaml"
        },
        {
          "glob": ".github/workflows/*.yml"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "grammar": "yaml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "diff",
      "scope": "source.diff",
      "fileTypes": [
        "diff",
        "patch",
        "rej"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "diff",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "git-rebase",
      "scope": "source.gitrebase",
      "fileTypes": [
        {
          "glob": "git-rebase-todo"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "git-rebase",
      "indent": {
        "tabWidth": 2,
        "unit": "\t"
      }
    },
    {
      "name": "regex",
      "scope": "source.regex",
      "fileTypes": [
        "regex",
        {
          "glob": ".Rbuildignore"
        }
      ],
      "injectionRegex": "regex"
    },
    {
      "name": "git-config",
      "scope": "source.gitconfig",
      "fileTypes": [
        "gitconfig",
        {
          "glob": ".gitmodules"
        },
        {
          "glob": "gitconfig"
        },
        {
          "glob": ".gitconfig"
        },
        {
          "glob": ".git/config"
        },
        {
          "glob": ".config/git/config"
        },
        {
          "glob": ".git/modules/*/config"
        },
        {
          "glob": "config.worktree"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "git-config",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "git-attributes",
      "scope": "source.gitattributes",
      "fileTypes": [
        {
          "glob": ".gitattributes"
        },
        {
          "glob": ".config/git/attributes"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "git-attributes",
      "grammar": "gitattributes"
    },
    {
      "name": "git-ignore",
      "scope": "source.gitignore",
      "fileTypes": [
        {
          "glob": ".gitignore_global"
        },
        {
          "glob": "git/ignore"
        },
        {
          "glob": ".git/info/exclude"
        },
        {
          "glob": ".ignore"
        },
        {
          "glob": "CODEOWNERS"
        },
        {
          "glob": ".config/helix/ignore"
        },
        {
          "glob": ".helix/ignore"
        },
        {
          "glob": ".*ignore"
        },
        {
          "glob": ".git-blame-ignore-revs"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "git-ignore",
      "grammar": "gitignore"
    },
    {
      "name": "graphql",
      "scope": "source.graphql",
      "fileTypes": [
        "gql",
        "graphql",
        "graphqls"
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "\"\"\"",
          "end": "\"\"\""
        }
      ],
      "injectionRegex": "graphql",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "elm",
      "scope": "source.elm",
      "fileTypes": [
        "elm"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "{-",
          "end": "-}"
        }
      ],
      "injectionRegex": "elm",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "iex",
      "scope": "source.iex",
      "fileTypes": [
        "iex"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "iex"
    },
    {
      "name": "rescript",
      "scope": "source.rescript",
      "fileTypes": [
        "res"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "rescript",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "erlang",
      "scope": "source.erlang",
      "fileTypes": [
        "erl",
        "hrl",
        "app",
        {
          "glob": "rebar.config"
        },
        {
          "glob": "rebar.lock"
        },
        {
          "glob": "*.app.src"
        }
      ],
      "shebangs": [
        "escript"
      ],
      "commentTokens": [
        "%%"
      ],
      "injectionRegex": "erl(ang)?",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "kotlin",
      "scope": "source.kotlin",
      "fileTypes": [
        "kt",
        "kts"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "hcl",
      "scope": "source.hcl",
      "fileTypes": [
        "hcl",
        "tf",
        "nomad"
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "(hcl|tf|nomad)",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "tfvars",
      "scope": "source.tfvars",
      "fileTypes": [
        "tfvars"
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "grammar": "hcl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "org",
      "scope": "source.org",
      "fileTypes": [
        "org"
      ],
      "injectionRegex": "org",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "solidity",
      "scope": "source.sol",
      "fileTypes": [
        "sol"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "(sol|solidity)",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "gleam",
      "scope": "source.gleam",
      "fileTypes": [
        "gleam"
      ],
      "commentTokens": [
        "//",
        "///",
        "////"
      ],
      "injectionRegex": "gleam",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "quarto",
      "scope": "source.qmd",
      "fileTypes": [
        "qmd"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "qmd",
      "grammar": "markdown",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "ron",
      "scope": "source.ron",
      "fileTypes": [
        "ron"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "ron",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "robot",
      "scope": "source.robot",
      "fileTypes": [
        "robot",
        "resource"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "robot",
      "indent": {
        "tabWidth": 4,
        "unit": " "
      }
    },
    {
      "name": "r",
      "scope": "source.r",
      "fileTypes": [
        "r",
        "R",
        {
          "glob": ".Rprofile"
        },
        {
          "glob": "Rprofile.site"
        },
        {
          "glob": ".RHistory"
        }
      ],
      "shebangs": [
        "r",
        "R"
      ],
      "commentTokens": [
        "#",
        "#'"
      ],
      "injectionRegex": "(r|R)",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "rmarkdown",
      "scope": "source.rmd",
      "fileTypes": [
        "rmd",
        "Rmd"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "(r|R)md",
      "grammar": "markdown",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "swift",
      "scope": "source.swift",
      "fileTypes": [
        "swift",
        "swiftinterface"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "swift"
    },
    {
      "name": "erb",
      "scope": "text.html.erb",
      "fileTypes": [
        "erb"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "erb",
      "grammar": "embedded-template",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "ejs",
      "scope": "text.html.ejs",
      "fileTypes": [
        "ejs"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "ejs",
      "grammar": "embedded-template",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "eex",
      "scope": "source.eex",
      "fileTypes": [
        "eex"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "eex",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "heex",
      "scope": "source.heex",
      "fileTypes": [
        "heex"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "heex",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "sql",
      "scope": "source.sql",
      "fileTypes": [
        "sql",
        "dsql"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "sql",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "gdscript",
      "scope": "source.gdscript",
      "fileTypes": [
        "gd"
      ],
      "commentTokens": [
        "#",
        "##"
      ],
      "injectionRegex": "gdscript",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "godot-resource",
      "scope": "source.tscn",
      "fileTypes": [
        "tscn",
        "tres",
        "godot",
        "gdextension"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "godot",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "nu",
      "scope": "source.nu",
      "fileTypes": [
        "nu",
        "nuon"
      ],
      "shebangs": [
        "nu"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "nu",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "vala",
      "scope": "source.vala",
      "fileTypes": [
        "vala",
        "vapi"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "vala",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "hare",
      "scope": "source.hare",
      "fileTypes": [
        "ha"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "hare",
      "indent": {
        "tabWidth": 8,
        "unit": "\t"
      }
    },
    {
      "name": "devicetree",
      "scope": "source.devicetree",
      "fileTypes": [
        "dts",
        "dtsi"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "(dtsi?|devicetree|fdt)",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "cairo",
      "scope": "source.cairo",
      "fileTypes": [
        "cairo"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "cairo",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "cpon",
      "scope": "scope.cpon",
      "fileTypes": [
        "cpon",
        "cp"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "cpon",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "odin",
      "scope": "source.odin",
      "fileTypes": [
        "odin"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "meson",
      "scope": "source.meson",
      "fileTypes": [
        {
          "glob": "meson.build"
        },
        {
          "glob": "meson.options"
        },
        {
          "glob": "meson_options.txt"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "meson",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "sshclientconfig",
      "scope": "source.sshclientconfig",
      "fileTypes": [
        {
          "glob": ".ssh/config"
        },
        {
          "glob": "/etc/ssh/ssh_config"
        },
        {
          "glob": "ssh_config.d/*.conf"
        }
      ],
      "commentTokens": [
        "#"
      ]
    },
    {
      "name": "scheme",
      "scope": "source.scheme",
      "fileTypes": [
        "ss",
        "scm",
        "sld"
      ],
      "shebangs": [
        "scheme",
        "guile",
        "chicken"
      ],
      "commentTokens": [
        ";"
      ],
      "blockCommentTokens": [
        {
          "start": "#|",
          "end": "|#"
        }
      ],
      "injectionRegex": "scheme",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "v",
      "scope": "source.v",
      "fileTypes": [
        "v",
        "vv",
        "vsh"
      ],
      "shebangs": [
        "v run"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "verilog",
      "scope": "source.verilog",
      "fileTypes": [
        {
          "glob": "*.v"
        },
        "vh"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "verilog",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "systemverilog",
      "scope": "source.systemverilog",
      "fileTypes": [
        "sv",
        "svh"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "edoc",
      "scope": "source.edoc",
      "fileTypes": [
        "edoc",
        "edoc.in"
      ],
      "injectionRegex": "edoc",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "jsdoc",
      "scope": "source.jsdoc",
      "fileTypes": [
        "jsdoc"
      ],
      "injectionRegex": "jsdoc",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "openscad",
      "scope": "source.openscad",
      "fileTypes": [
        "scad"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "openscad",
      "indent": {
        "tabWidth": 2,
        "unit": "\t"
      }
    },
    {
      "name": "prisma",
      "scope": "source.prisma",
      "fileTypes": [
        "prisma"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "prisma",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "clojure",
      "scope": "source.clojure",
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
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "(clojure|clj|edn|boot)",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "starlark",
      "scope": "source.starlark",
      "fileTypes": [
        "bzl",
        "bazel",
        "star",
        {
          "glob": "BUILD"
        },
        {
          "glob": "BUCK"
        },
        {
          "glob": "BUILD.*"
        },
        {
          "glob": "WORKSPACE"
        },
        {
          "glob": "WORKSPACE.bzlmod"
        },
        {
          "glob": "PACKAGE"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "(starlark|bzl|bazel|buck)",
      "grammar": "python",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "elvish",
      "scope": "source.elvish",
      "fileTypes": [
        "elv"
      ],
      "shebangs": [
        "elvish"
      ],
      "commentTokens": [
        "#"
      ],
      "grammar": "elvish",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "idris",
      "scope": "source.idr",
      "fileTypes": [
        "idr"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "{-",
          "end": "-}"
        }
      ],
      "injectionRegex": "idr",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "fortran",
      "scope": "source.fortran",
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
      "commentTokens": [
        "!"
      ],
      "injectionRegex": "fortran",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "ungrammar",
      "scope": "source.ungrammar",
      "fileTypes": [
        "ungram",
        "ungrammar"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "ungrammar",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "dot",
      "scope": "source.dot",
      "fileTypes": [
        "dot"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "dot",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "cue",
      "scope": "source.cue",
      "fileTypes": [
        "cue"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "cue",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "slang",
      "scope": "source.lang",
      "fileTypes": [
        "slang"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "slang",
      "indent": {
        "tabWidth": 4,
        "unit": "  "
      }
    },
    {
      "name": "slint",
      "scope": "source.slint",
      "fileTypes": [
        "slint"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "slint",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "task",
      "scope": "source.task",
      "fileTypes": [
        "task"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "task",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "xit",
      "scope": "source.xit",
      "fileTypes": [
        "xit"
      ],
      "injectionRegex": "xit",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "esdl",
      "scope": "source.esdl",
      "fileTypes": [
        "gel",
        "esdl"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "esdl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "pascal",
      "scope": "source.pascal",
      "fileTypes": [
        "pas",
        "pp",
        {
          "glob": "*.inc"
        },
        "lpr",
        "lfm"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "{",
          "end": "}"
        }
      ],
      "injectionRegex": "pascal",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "sml",
      "scope": "source.sml",
      "fileTypes": [
        "sml"
      ],
      "blockCommentTokens": [
        {
          "start": "(*",
          "end": "*)"
        }
      ],
      "injectionRegex": "sml"
    },
    {
      "name": "jsonnet",
      "scope": "source.jsonnet",
      "fileTypes": [
        "libsonnet",
        "jsonnet"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "ada",
      "scope": "source.ada",
      "fileTypes": [
        "adb",
        "ads"
      ],
      "commentTokens": [
        "--"
      ],
      "injectionRegex": "ada",
      "indent": {
        "tabWidth": 3,
        "unit": "   "
      }
    },
    {
      "name": "astro",
      "scope": "source.astro",
      "fileTypes": [
        "astro"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "astro",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "bass",
      "scope": "source.bass",
      "fileTypes": [
        "bass"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "bass",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "wat",
      "scope": "source.wat",
      "fileTypes": [
        "wat"
      ],
      "commentTokens": [
        ";;"
      ],
      "blockCommentTokens": [
        {
          "start": "(;",
          "end": ";)"
        }
      ]
    },
    {
      "name": "wast",
      "scope": "source.wast",
      "fileTypes": [
        "wast"
      ],
      "commentTokens": [
        ";;"
      ],
      "blockCommentTokens": [
        {
          "start": "(;",
          "end": ";)"
        }
      ]
    },
    {
      "name": "d",
      "scope": "source.d",
      "fileTypes": [
        "d",
        "dd"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "d",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "vhs",
      "scope": "source.vhs",
      "fileTypes": [
        "tape"
      ],
      "commentTokens": [
        "#"
      ],
      "grammar": "vhs",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "kdl",
      "scope": "source.kdl",
      "fileTypes": [
        "kdl"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "kdl"
    },
    {
      "name": "xml",
      "scope": "source.xml",
      "fileTypes": [
        "ascx",
        "atom",
        "axaml",
        "axml",
        "bpmn",
        "checkstyle",
        "cpt",
        "csl",
        "csproj.user",
        "dita",
        "ditamap",
        "dtml",
        "fods",
        "fodt",
        "fxml",
        "gir",
        "glif",
        "gml",
        "gpx",
        "iml",
        "isml",
        "itermcolors",
        "jmx",
        "kml",
        "launch",
        "menu",
        "mobileconfig",
        "mpd",
        "musicxml",
        "mxml",
        "ncx",
        "nuspec",
        "opml",
        "osc",
        "osm",
        "plist",
        "policy",
        "pt",
        "publishsettings",
        "pubxml",
        "pubxml.user",
        "rbxlx",
        "rbxmx",
        "resx",
        "rng",
        "rss",
        "shproj",
        "smil",
        "storyboard",
        "sublime-snippet",
        "svg",
        "terminal",
        "tld",
        "tmx",
        "ui",
        "vbproj.user",
        "vcxproj",
        "vcxproj.filters",
        "wixproj",
        "wsdl",
        "wxi",
        "wxs",
        "xaml",
        "xbl",
        "xib",
        "xlf",
        "xliff",
        "xml",
        "xmp",
        "xoml",
        "xpdl",
        "xrc",
        "xsd",
        "xsl",
        "xul",
        {
          "glob": "*.tm[Ll]anguage"
        },
        {
          "glob": "*.tm[Pp]references"
        },
        {
          "glob": "*.tm[Tt]heme"
        }
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "xml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "dtd",
      "scope": "source.dtd",
      "fileTypes": [
        "dtd",
        "ent"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "dtd",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "wit",
      "scope": "source.wit",
      "fileTypes": [
        "wit"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "wit",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "env",
      "scope": "source.env",
      "fileTypes": [
        {
          "glob": ".env"
        },
        {
          "glob": ".env.*"
        },
        {
          "glob": ".envrc"
        },
        {
          "glob": ".envrc.*"
        },
        {
          "glob": "_environment"
        },
        {
          "glob": "_environment-*"
        },
        {
          "glob": "_environment.local"
        },
        {
          "glob": "_environment.required"
        },
        {
          "glob": ".bazeliskrc"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "env",
      "grammar": "bash",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "systemd",
      "scope": "source.ini",
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
        {
          "glob": "systemd/**/*.conf"
        },
        "container",
        {
          "glob": "containers/systemd/**/*.pod"
        },
        "kube",
        "network",
        "volume",
        "build",
        "image"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "systemd",
      "grammar": "ini"
    },
    {
      "name": "ini",
      "scope": "source.ini",
      "fileTypes": [
        "ini",
        "desktop",
        {
          "glob": "mimeapps.list"
        },
        {
          "glob": ".editorconfig"
        },
        {
          "glob": ".npmrc"
        },
        {
          "glob": "hgrc"
        },
        {
          "glob": "npmrc"
        },
        {
          "glob": "rclone.conf"
        },
        {
          "glob": ".aws/config"
        },
        "properties",
        "cfg",
        "directory",
        {
          "glob": ".wslconfig"
        },
        {
          "glob": ".buckconfig"
        },
        {
          "glob": ".buckconfig.local"
        },
        {
          "glob": ".buckconfig.d/*"
        },
        {
          "glob": "/etc/buckconfig"
        },
        {
          "glob": "/etc/buckconfig.d/*"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "ini",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "inko",
      "scope": "source.inko",
      "fileTypes": [
        "inko"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "inko",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "bicep",
      "scope": "source.bicep",
      "fileTypes": [
        "bicep",
        "bicepparam"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": " "
      }
    },
    {
      "name": "qml",
      "scope": "source.qml",
      "fileTypes": [
        "qml"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "grammar": "qmljs",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "mermaid",
      "scope": "source.mermaid",
      "fileTypes": [
        "mermaid",
        "mmd"
      ],
      "commentTokens": [
        "%%"
      ],
      "injectionRegex": "mermaid",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "matlab",
      "scope": "source.m",
      "fileTypes": [
        "m"
      ],
      "shebangs": [
        "octave-cli",
        "matlab"
      ],
      "commentTokens": [
        "%"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "ponylang",
      "scope": "source.pony",
      "fileTypes": [
        "pony"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "pony",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "dhall",
      "scope": "source.dhall",
      "fileTypes": [
        "dhall"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "{-",
          "end": "-}"
        }
      ],
      "injectionRegex": "dhall",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "sage",
      "scope": "source.sage",
      "fileTypes": [
        "sage"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "sage",
      "grammar": "python",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "msbuild",
      "scope": "source.msbuild",
      "fileTypes": [
        "proj",
        "vbproj",
        "csproj",
        "fsproj",
        "targets",
        "props"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "msbuild",
      "grammar": "xml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "pem",
      "scope": "source.pem",
      "fileTypes": [
        "pem",
        "cert",
        "crt"
      ],
      "injectionRegex": "pem",
      "grammar": "pem"
    },
    {
      "name": "passwd",
      "scope": "source.passwd",
      "fileTypes": [
        {
          "glob": "passwd"
        }
      ]
    },
    {
      "name": "hosts",
      "scope": "source.hosts",
      "fileTypes": [
        {
          "glob": "hosts"
        }
      ],
      "commentTokens": [
        "#"
      ]
    },
    {
      "name": "uxntal",
      "scope": "source.tal",
      "fileTypes": [
        "tal"
      ],
      "blockCommentTokens": [
        {
          "start": "(",
          "end": ")"
        }
      ],
      "injectionRegex": "tal"
    },
    {
      "name": "yuck",
      "scope": "source.yuck",
      "fileTypes": [
        "yuck"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "yuck",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "prql",
      "scope": "source.prql",
      "fileTypes": [
        "prql"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "prql",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "po",
      "scope": "source.po",
      "fileTypes": [
        "po",
        "pot"
      ],
      "commentTokens": [
        "#"
      ]
    },
    {
      "name": "nasm",
      "scope": "source.nasm",
      "fileTypes": [
        "asm",
        "S",
        "nasm"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "n?asm",
      "indent": {
        "tabWidth": 8,
        "unit": "        "
      }
    },
    {
      "name": "gas",
      "scope": "source.gas",
      "fileTypes": [
        "s"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "gas",
      "indent": {
        "tabWidth": 8,
        "unit": "        "
      }
    },
    {
      "name": "rst",
      "scope": "source.rst",
      "fileTypes": [
        "rst"
      ],
      "commentTokens": [
        ".."
      ]
    },
    {
      "name": "capnp",
      "scope": "source.capnp",
      "fileTypes": [
        "capnp"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "capnp",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "smithy",
      "scope": "source.smithy",
      "fileTypes": [
        "smithy"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "smithy",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "hdl",
      "scope": "source.hdl",
      "fileTypes": [
        "hdl"
      ],
      "injectionRegex": "hdl",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "vhdl",
      "scope": "source.vhdl",
      "fileTypes": [
        "vhd",
        "vhdl"
      ],
      "commentTokens": [
        "--"
      ],
      "injectionRegex": "vhdl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "rego",
      "scope": "source.rego",
      "fileTypes": [
        "rego"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "rego",
      "grammar": "rego"
    },
    {
      "name": "nim",
      "scope": "source.nim",
      "fileTypes": [
        "nim",
        "nims",
        "nimble"
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "#[",
          "end": "]#"
        }
      ],
      "injectionRegex": "nim",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "cabal",
      "scope": "source.cabal",
      "fileTypes": [
        "cabal"
      ],
      "commentTokens": [
        "--"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "hurl",
      "scope": "source.hurl",
      "fileTypes": [
        "hurl"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "hurl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "markdoc",
      "scope": "text.markdoc",
      "fileTypes": [
        "mdoc"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ]
    },
    {
      "name": "opencl",
      "scope": "source.cl",
      "fileTypes": [
        {
          "glob": "*.cl"
        }
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "(cl|opencl)"
    },
    {
      "name": "just",
      "scope": "source.just",
      "fileTypes": [
        "just",
        {
          "glob": "justfile"
        },
        {
          "glob": "Justfile"
        },
        {
          "glob": ".justfile"
        },
        {
          "glob": ".Justfile"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "just",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "gn",
      "scope": "source.gn",
      "fileTypes": [
        "gn",
        "gni"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "gn",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "blueprint",
      "scope": "source.blueprint",
      "fileTypes": [
        "blp"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "blueprint",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "forth",
      "scope": "source.forth",
      "fileTypes": [
        "fs",
        "forth",
        "fth",
        "4th"
      ],
      "commentTokens": [
        "\\"
      ],
      "injectionRegex": "forth",
      "indent": {
        "tabWidth": 3,
        "unit": "   "
      }
    },
    {
      "name": "fsharp",
      "scope": "source.fs",
      "fileTypes": [
        {
          "glob": "*.fs"
        },
        "fsx",
        "fsi",
        "fsscript"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "(*",
          "end": "*)"
        }
      ],
      "injectionRegex": "fsharp",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "t32",
      "scope": "source.t32",
      "fileTypes": [
        "cmm",
        "t32"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "t32",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "webc",
      "scope": "text.html.webc",
      "fileTypes": [
        "webc"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "injectionRegex": "webc",
      "grammar": "html",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "typst",
      "scope": "source.typst",
      "fileTypes": [
        "typst",
        "typ"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "typ(st)?",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "nunjucks",
      "scope": "text.html.nunjucks",
      "fileTypes": [
        "njk"
      ],
      "blockCommentTokens": [
        {
          "start": "{#",
          "end": "#}"
        }
      ],
      "injectionRegex": "nunjucks",
      "grammar": "jinja2",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "jinja",
      "scope": "text.html.jinja",
      "fileTypes": [
        "jinja",
        "jinja2",
        "j2"
      ],
      "blockCommentTokens": [
        {
          "start": "{#",
          "end": "#}"
        }
      ],
      "injectionRegex": "jinja",
      "grammar": "jinja2",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "jjconfig",
      "scope": "source.jjconfig",
      "fileTypes": [
        {
          "glob": "jj/config.toml"
        },
        {
          "glob": "jj/**/*.toml"
        },
        {
          "glob": ".jj/repo/*.toml"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "jjconfig",
      "grammar": "toml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "jjdescription",
      "scope": "jj.description",
      "fileTypes": [
        {
          "glob": "*.jjdescription"
        }
      ],
      "commentTokens": [
        "JJ:"
      ],
      "textWidth": 72,
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      },
      "rulers": [
        51,
        73
      ]
    },
    {
      "name": "jjrevset",
      "scope": "jj.revset",
      "fileTypes": [
        "jjrevset"
      ]
    },
    {
      "name": "jjtemplate",
      "scope": "jj.template",
      "fileTypes": [
        "jjtemplate"
      ]
    },
    {
      "name": "miseconfig",
      "scope": "source.miseconfig",
      "fileTypes": [
        {
          "glob": "mise.toml"
        },
        {
          "glob": ".mise.toml"
        },
        {
          "glob": "mise.*.toml"
        },
        {
          "glob": ".mise.*.toml"
        },
        {
          "glob": "mise/config.toml"
        },
        {
          "glob": ".mise/config.toml"
        },
        {
          "glob": ".config/mise.toml"
        },
        {
          "glob": ".config/mise/conf.d/*.toml"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "miseconfig",
      "grammar": "toml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "jq",
      "scope": "source.jq",
      "fileTypes": [
        "jq"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "jq",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "wren",
      "scope": "source.wren",
      "fileTypes": [
        "wren"
      ],
      "injectionRegex": "wren",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "unison",
      "scope": "source.unison",
      "fileTypes": [
        "u"
      ],
      "commentTokens": [
        "--"
      ],
      "injectionRegex": "unison",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "todotxt",
      "scope": "text.todotxt",
      "fileTypes": [
        {
          "glob": "*.todo.txt"
        },
        "todotxt"
      ]
    },
    {
      "name": "strace",
      "scope": "source.strace",
      "fileTypes": [
        "strace"
      ]
    },
    {
      "name": "gemini",
      "scope": "source.gmi",
      "fileTypes": [
        "gmi"
      ]
    },
    {
      "name": "agda",
      "scope": "source.agda",
      "fileTypes": [
        "agda"
      ],
      "commentTokens": [
        "--"
      ],
      "injectionRegex": "agda",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "templ",
      "scope": "source.templ",
      "fileTypes": [
        "templ"
      ],
      "commentTokens": [
        "//"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "dbml",
      "scope": "source.dbml",
      "fileTypes": [
        "dbml"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "dbml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "bitbake",
      "scope": "source.bitbake",
      "fileTypes": [
        "bb",
        "bbappend",
        "bbclass",
        {
          "glob": "conf/*.conf"
        },
        {
          "glob": "conf/*/*.{inc,conf}"
        },
        {
          "glob": "recipe-*/*/*.inc"
        }
      ],
      "commentTokens": [
        "#"
      ]
    },
    {
      "name": "log",
      "scope": "source.log",
      "fileTypes": [
        "log"
      ]
    },
    {
      "name": "hoon",
      "scope": "source.hoon",
      "fileTypes": [
        "hoon"
      ],
      "commentTokens": [
        "::"
      ],
      "injectionRegex": "hoon",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "hocon",
      "scope": "source.conf",
      "fileTypes": [
        {
          "glob": "**/src/*/resources/**/*.conf"
        },
        {
          "glob": "*scalafmt*.conf"
        },
        {
          "glob": "*scalafix*.conf"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "koka",
      "scope": "source.koka",
      "fileTypes": [
        "kk"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "koka",
      "indent": {
        "tabWidth": 8,
        "unit": "  "
      }
    },
    {
      "name": "tact",
      "scope": "source.tact",
      "fileTypes": [
        "tact"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "tact",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "pkl",
      "scope": "source.pkl",
      "fileTypes": [
        "pkl",
        "pcf",
        {
          "glob": "PklProject"
        }
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "pkl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "groovy",
      "scope": "source.groovy",
      "fileTypes": [
        "gradle",
        "groovy",
        "jenkinsfile",
        {
          "glob": "Jenkinsfile"
        },
        {
          "glob": "Jenkinsfile.*"
        }
      ],
      "shebangs": [
        "groovy"
      ],
      "commentTokens": [
        "//"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "fidl",
      "scope": "source.fidl",
      "fileTypes": [
        "fidl"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "fidl",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "powershell",
      "scope": "source.powershell",
      "fileTypes": [
        "ps1",
        "psm1",
        "psd1",
        "pscc",
        "psrc"
      ],
      "shebangs": [
        "pwsh",
        "powershell"
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "<#",
          "end": "#>"
        }
      ],
      "injectionRegex": "(pwsh|powershell)",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "ld",
      "scope": "source.ld",
      "fileTypes": [
        "ld"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "ld",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "hy",
      "scope": "source.hy",
      "fileTypes": [
        "hy"
      ],
      "commentTokens": [
        ";"
      ],
      "grammar": "scheme",
      "indent": {
        "tabWidth": 1,
        "unit": " "
      }
    },
    {
      "name": "hyprlang",
      "scope": "source.hyprlang",
      "fileTypes": [
        {
          "glob": "hypr/*.conf"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "grammar": "hyprlang"
    },
    {
      "name": "tcl",
      "scope": "source.tcl",
      "fileTypes": [
        "tcl"
      ],
      "shebangs": [
        "tclsh",
        "tclish",
        "jimsh",
        "wish"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "tcl"
    },
    {
      "name": "supercollider",
      "scope": "source.supercollider",
      "fileTypes": [
        "scd",
        {
          "glob": "*.sc"
        },
        "quark"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "supercollider",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "rpmspec",
      "scope": "source.rpmspec",
      "fileTypes": [
        "spec"
      ],
      "commentTokens": [
        "#"
      ]
    },
    {
      "name": "pkgbuild",
      "scope": "source.bash",
      "fileTypes": [
        {
          "glob": "PKGBUILD"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "grammar": "bash"
    },
    {
      "name": "helm",
      "scope": "source.helm",
      "fileTypes": [
        {
          "glob": "templates/*.yaml"
        },
        {
          "glob": "templates/*.yml"
        },
        {
          "glob": "templates/_*.tpl"
        },
        {
          "glob": "templates/NOTES.txt"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "grammar": "gotmpl"
    },
    {
      "name": "glimmer",
      "scope": "source.glimmer",
      "fileTypes": [
        {
          "glob": "{app,addon}/{components,templates}/*.hbs"
        }
      ],
      "blockCommentTokens": [
        {
          "start": "{{!",
          "end": "}}"
        }
      ],
      "injectionRegex": "hbs",
      "grammar": "glimmer"
    },
    {
      "name": "ohm",
      "scope": "source.ohm",
      "fileTypes": [
        "ohm"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        },
        {
          "start": "/**",
          "end": "*/"
        }
      ],
      "injectionRegex": "ohm",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "earthfile",
      "scope": "source.earthfile",
      "fileTypes": [
        {
          "glob": "Earthfile"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "earthfile",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "adl",
      "scope": "source.adl",
      "fileTypes": [
        "adl"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "adl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "ldif",
      "scope": "source.ldif",
      "fileTypes": [
        "ldif"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "ldif"
    },
    {
      "name": "xtc",
      "scope": "source.xtc",
      "fileTypes": [
        "xtc",
        "xpc",
        "xoa"
      ],
      "commentTokens": [
        ";"
      ]
    },
    {
      "name": "move",
      "scope": "source.move",
      "fileTypes": [
        "move"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "move",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "pest",
      "scope": "source.pest",
      "fileTypes": [
        "pest"
      ],
      "commentTokens": [
        "//",
        "///",
        "//!"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "pest",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "elisp",
      "scope": "source.elisp",
      "fileTypes": [
        "el"
      ],
      "commentTokens": [
        ";"
      ]
    },
    {
      "name": "gjs",
      "scope": "source.gjs",
      "fileTypes": [
        "gjs"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "grammar": "glimmer-javascript",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "gts",
      "scope": "source.gts",
      "fileTypes": [
        "gts"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "grammar": "glimmer-typescript",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "gherkin",
      "scope": "source.feature",
      "fileTypes": [
        "feature"
      ],
      "commentTokens": [
        "#"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "thrift",
      "scope": "source.thrift",
      "fileTypes": [
        "thrift"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "circom",
      "scope": "source.circom",
      "fileTypes": [
        "circom"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "circom",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "snakemake",
      "scope": "source.snakemake",
      "fileTypes": [
        "smk",
        {
          "glob": "Snakefile"
        }
      ],
      "commentTokens": [
        "#",
        "##"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "cylc",
      "scope": "source.cylc",
      "fileTypes": [
        "cylc",
        {
          "glob": "suite.rc"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "cylc",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "quint",
      "scope": "source.quint",
      "fileTypes": [
        "qnt"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "spade",
      "scope": "source.spade",
      "fileTypes": [
        "spade"
      ],
      "commentTokens": [
        "//",
        "///"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        },
        {
          "start": "/**",
          "end": "*/"
        }
      ],
      "injectionRegex": "spade",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "amber",
      "scope": "source.ab",
      "fileTypes": [
        "ab"
      ],
      "commentTokens": [
        "//",
        "///"
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "koto",
      "scope": "source.koto",
      "fileTypes": [
        "koto"
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "#-",
          "end": "-#"
        }
      ],
      "injectionRegex": "koto",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "gpr",
      "scope": "source.gpr",
      "fileTypes": [
        "gpr"
      ],
      "commentTokens": [
        "--"
      ],
      "injectionRegex": "gpr",
      "indent": {
        "tabWidth": 3,
        "unit": "   "
      }
    },
    {
      "name": "vento",
      "scope": "text.html.vto",
      "fileTypes": [
        "vto"
      ],
      "blockCommentTokens": [
        {
          "start": "{{#",
          "end": "#}}"
        }
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "nginx",
      "scope": "source.nginx",
      "fileTypes": [
        {
          "glob": "sites-available/*.conf"
        },
        {
          "glob": "sites-enabled/*.conf"
        },
        {
          "glob": "nginx.conf"
        },
        {
          "glob": "conf.d/*.conf"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "nginx",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "codeql",
      "scope": "source.ql",
      "fileTypes": [
        "ql",
        "qll"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "codeql",
      "grammar": "ql",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "gren",
      "scope": "source.gren",
      "fileTypes": [
        "gren"
      ],
      "commentTokens": [
        "--"
      ],
      "blockCommentTokens": [
        {
          "start": "{-",
          "end": "-}"
        }
      ],
      "injectionRegex": "gren",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "tera",
      "scope": "source.tera",
      "fileTypes": [
        "tera"
      ],
      "blockCommentTokens": [
        {
          "start": "{#",
          "end": "#}"
        },
        {
          "start": "{#-",
          "end": "-#}"
        },
        {
          "start": "{#",
          "end": "-#}"
        },
        {
          "start": "{#-",
          "end": "#}"
        }
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "fga",
      "scope": "source.fga",
      "fileTypes": [
        "fga",
        {
          "glob": "fga.mod"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "fga",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "csv",
      "scope": "source.csv",
      "fileTypes": [
        "csv"
      ]
    },
    {
      "name": "yara",
      "scope": "source.yara",
      "fileTypes": [
        "yara",
        "yar"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "ink",
      "scope": "source.ink",
      "fileTypes": [
        "ink"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "ink",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "sourcepawn",
      "scope": "source.sourcepawn",
      "fileTypes": [
        "sp",
        {
          "glob": "*.inc"
        }
      ],
      "commentTokens": [
        "//"
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "  "
      }
    },
    {
      "name": "vim",
      "scope": "source.vim",
      "fileTypes": [
        "vim",
        {
          "glob": ".vimrc"
        },
        {
          "glob": ".nvimrc"
        },
        {
          "glob": ".exrc"
        }
      ],
      "commentTokens": [
        "\""
      ],
      "injectionRegex": "vim",
      "indent": {
        "tabWidth": 4,
        "unit": "\t"
      }
    },
    {
      "name": "tlaplus",
      "scope": "scope.tlaplus",
      "fileTypes": [
        "tla"
      ],
      "commentTokens": [
        "\\*"
      ],
      "blockCommentTokens": [
        {
          "start": "(*",
          "end": "*)"
        }
      ],
      "injectionRegex": "tla",
      "indent": {
        "tabWidth": 4,
        "unit": " "
      }
    },
    {
      "name": "werk",
      "scope": "source.werk",
      "fileTypes": [
        "werk",
        {
          "glob": "Werkfile"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "debian",
      "scope": "text.debian",
      "fileTypes": [
        "dsc",
        "changes",
        {
          "glob": "debian/**/control"
        },
        {
          "glob": "etc/apt/sources.list.d/*.sources"
        }
      ],
      "commentTokens": [
        "#"
      ]
    },
    {
      "name": "pug",
      "scope": "source.pug",
      "fileTypes": [
        "pug"
      ],
      "commentTokens": [
        "//",
        "//-"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "dunstrc",
      "scope": "source.dunstrc",
      "fileTypes": [
        {
          "glob": "dunst/dunstrc"
        }
      ],
      "commentTokens": [
        "#",
        ";"
      ]
    },
    {
      "name": "rust-format-args",
      "scope": "source.rust-format-args",
      "fileTypes": [],
      "injectionRegex": "rust-format-args"
    },
    {
      "name": "rust-format-args-macro",
      "scope": "source.rust-format-args-macro",
      "fileTypes": [],
      "grammar": "rust"
    },
    {
      "name": "clarity",
      "scope": "source.clar",
      "fileTypes": [
        "clar"
      ],
      "commentTokens": [
        ";;"
      ],
      "injectionRegex": "clarity",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "alloy",
      "scope": "source.alloy",
      "fileTypes": [
        "alloy"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "alloy",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "luau",
      "scope": "source.luau",
      "fileTypes": [
        "luau"
      ],
      "commentTokens": [
        "--",
        "---"
      ],
      "blockCommentTokens": [
        {
          "start": "--[[",
          "end": "]]"
        },
        {
          "start": "--[=[",
          "end": "]=]"
        },
        {
          "start": "--[==[",
          "end": "]==]"
        }
      ],
      "injectionRegex": "^luau$",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "caddyfile",
      "scope": "source.caddyfile",
      "fileTypes": [
        "Caddyfile",
        {
          "glob": "Caddyfile"
        },
        {
          "glob": "Caddyfile.*"
        },
        "caddyfile",
        {
          "glob": "caddyfile"
        },
        {
          "glob": "caddyfile.*"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "caddyfile",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "properties",
      "scope": "source.properties",
      "fileTypes": [
        {
          "glob": "*.properties"
        },
        "prefs"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "properties"
    },
    {
      "name": "robots.txt",
      "scope": "source.robots.txt",
      "fileTypes": [
        {
          "glob": "robots.txt"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "robots[\\.-]txt",
      "grammar": "robots"
    },
    {
      "name": "pip-requirements",
      "scope": "source.pip-requirements",
      "fileTypes": [
        {
          "glob": "requirements.txt"
        },
        {
          "glob": "constraints.txt"
        }
      ],
      "injectionRegex": "(pip-)?requirements(\\.txt)?",
      "grammar": "requirements"
    },
    {
      "name": "kconfig",
      "scope": "source.kconfig",
      "fileTypes": [
        "Kconfig",
        {
          "glob": "Kconfig.*"
        }
      ]
    },
    {
      "name": "doxyfile",
      "scope": "source.doxyfile",
      "fileTypes": [
        {
          "glob": "Doxyfile"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "[Dd]oxyfile",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "cross-config",
      "scope": "source.cross-config",
      "fileTypes": [
        {
          "glob": "Cross.toml"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "cross(-config)",
      "grammar": "toml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "git-cliff-config",
      "scope": "source.git-cliff-config",
      "fileTypes": [
        {
          "glob": "cliff.toml"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "git-cliff(-config)",
      "grammar": "toml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "cython",
      "scope": "source.cython",
      "fileTypes": [
        "pxd",
        "pxi",
        "pyx"
      ],
      "commentTokens": [
        "#"
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "shellcheckrc",
      "scope": "source.shellcheckrc",
      "fileTypes": [
        {
          "glob": "shellcheckrc"
        },
        {
          "glob": ".shellcheckrc"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "shellcheck(rc)?"
    },
    {
      "name": "strictdoc",
      "scope": "source.strictdoc",
      "fileTypes": [
        "sdoc",
        "sgra"
      ],
      "commentTokens": [
        ".."
      ],
      "injectionRegex": "strictdoc"
    },
    {
      "name": "docker-bake",
      "scope": "source.docker-bake",
      "fileTypes": [
        {
          "glob": "docker-bake.hcl"
        },
        {
          "glob": "docker-bake.override.hcl"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "docker-bake",
      "grammar": "hcl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "gitlab-ci",
      "scope": "source.gitlab-ci",
      "fileTypes": [
        {
          "glob": ".gitlab-ci.yml"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "^gitlab-ci$",
      "grammar": "yaml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "wikitext",
      "scope": "source.wikitext",
      "fileTypes": [
        "wikimedia",
        "mediawiki",
        "wikitext"
      ],
      "blockCommentTokens": [
        {
          "start": "<!--",
          "end": "-->"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "slisp",
      "scope": "source.sl",
      "fileTypes": [
        "sl"
      ],
      "commentTokens": [
        ";"
      ],
      "injectionRegex": "sl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "nearley",
      "scope": "source.nearley",
      "fileTypes": [
        "ne"
      ],
      "commentTokens": [
        "#"
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "kcl",
      "scope": "source.kcl",
      "fileTypes": [
        "kcl"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "kcl",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "bovex",
      "scope": "source.bovex",
      "fileTypes": [
        "bovex",
        "bibvex"
      ],
      "blockCommentTokens": [
        {
          "start": "(*",
          "end": "*)"
        },
        {
          "start": "[*",
          "end": "*]"
        }
      ],
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "haxe",
      "scope": "source.haxe",
      "fileTypes": [
        "hx"
      ],
      "commentTokens": [
        "//",
        "///"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        },
        {
          "start": "/**",
          "end": "*/"
        }
      ],
      "injectionRegex": "hx|haxe",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "basic",
      "scope": "source.basic",
      "fileTypes": [
        "bas"
      ],
      "commentTokens": [
        "REM"
      ],
      "injectionRegex": "basic",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "freebasic",
      "scope": "source.freebasic",
      "fileTypes": [
        {
          "glob": "*.bas"
        },
        "bi"
      ],
      "commentTokens": [
        "'",
        "REM"
      ],
      "injectionRegex": "freebasic",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "scfg",
      "scope": "source.scfg",
      "fileTypes": [
        {
          "glob": "kanshi/config"
        }
      ],
      "commentTokens": [
        "#"
      ]
    },
    {
      "name": "ripple",
      "scope": "source.ripple",
      "fileTypes": [
        "ripple"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "woodpecker-ci",
      "scope": "source.woodpecker-ci",
      "fileTypes": [
        {
          "glob": ".woodpecker.yaml"
        },
        {
          "glob": ".woodpecker/*.yaml"
        },
        {
          "glob": ".woodpecker/*.yml"
        }
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "^woodpecker-ci$",
      "grammar": "yaml",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "chuck",
      "scope": "source.ck",
      "fileTypes": [
        {
          "glob": "*.ck"
        }
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "^chuck$|^ck$",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "qmv",
      "scope": "source.qmv",
      "fileTypes": [
        {
          "glob": "/tmp/qmv*"
        }
      ],
      "indent": {
        "tabWidth": 8,
        "unit": "        "
      }
    },
    {
      "name": "klog",
      "scope": "source.klg",
      "fileTypes": [
        "klg"
      ],
      "injectionRegex": "klg",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "styx",
      "scope": "source.styx",
      "fileTypes": [
        "styx"
      ],
      "commentTokens": [
        "//",
        "///"
      ],
      "injectionRegex": "styx",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    },
    {
      "name": "tilt",
      "scope": "source.tilt",
      "fileTypes": [
        {
          "glob": "Tiltfile"
        },
        "tiltfile"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "[Tt]ilt(file)?",
      "grammar": "python",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "gnuplot",
      "scope": "source.gnuplot",
      "fileTypes": [
        "gnuplot",
        "plot",
        "plt"
      ],
      "commentTokens": [
        "#"
      ],
      "injectionRegex": "gnuplot",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "ebnf",
      "scope": "source.ebnf",
      "fileTypes": [
        "ebnf"
      ],
      "blockCommentTokens": [
        {
          "end": "*)",
          "start": "(*"
        }
      ],
      "injectionRegex": "ebnf",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "drools",
      "scope": "source.drools",
      "fileTypes": [
        "drl"
      ],
      "commentTokens": [
        "//"
      ]
    },
    {
      "name": "proverif",
      "scope": "source.proverif",
      "fileTypes": [
        "pv"
      ],
      "blockCommentTokens": [
        {
          "start": "(*",
          "end": "*)"
        }
      ],
      "injectionRegex": "proverif",
      "indent": {
        "tabWidth": 8,
        "unit": "        "
      }
    },
    {
      "name": "ptx",
      "scope": "source.ptx",
      "fileTypes": [
        "ptx"
      ],
      "commentTokens": [
        "//"
      ],
      "blockCommentTokens": [
        {
          "start": "/*",
          "end": "*/"
        }
      ],
      "injectionRegex": "ptx",
      "indent": {
        "tabWidth": 4,
        "unit": "    "
      }
    },
    {
      "name": "tql",
      "scope": "source.tql",
      "fileTypes": [
        "tql"
      ],
      "commentTokens": [
        "//"
      ],
      "injectionRegex": "tql",
      "indent": {
        "tabWidth": 2,
        "unit": "  "
      }
    }
  ]
};
