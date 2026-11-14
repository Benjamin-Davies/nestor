# Nestor

A C LSP that is optimised for moderate to large codebases. It takes a silly amount of shortcuts but is still useful for LSP things.

To achieve this goal, Nestor uses [Tree Sitter](https://tree-sitter.github.io/tree-sitter/), a fast context-free parser that's often used for syntax highlighting.

Nestor is named after the [Kea](https://en.wikipedia.org/wiki/Kea) (*Nestor Notabilis*), a curious and clever green parrot found in the Southern Alps of New Zealand. It was inspired by [OpenGrok](https://oracle.github.io/opengrok/)'s naivety and flexibility.

**Goals:**
* Support basic LSP features (goto definition, find references, autocompletion)
* Basic scope awareness
* Efficient for large codebases
* Single rust binary
* UTF-8 native, but also supports UTF-16 clients

**Non-goals:**
* Type-aware analysis
* Preprocessor handling beyond goto definition for macros
* UTF-16 files
* Unicode normalization

## Usage

You will need [cargo](https://rustup.rs/) and an LSP-compatible editor (e.g. VS Code or Zed).

* Install the Nestor language server, e.g. `cargo install --git https://github.com/Benjamin-Davies/nestor`.
* Install the extension for your editor:
  * Zed: https://github.com/Benjamin-Davies/zed-nestor
  * If your editor isn't listed above, you'll need to manually configure the LSP (e.g. https://github.com/zed-industries/zed/discussions/24092#discussioncomment-15278796).
