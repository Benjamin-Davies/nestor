# Nestor

A C LSP that is optimised for moderate to large codebases. It takes a silly amount of shortcuts but is still useful for LSP things.

To achieve this goal, Nestor uses [Tree Sitter](https://tree-sitter.github.io/tree-sitter/), a fast context-free parser that's often used for syntax highlighting.

Nestor is named after the [Kea](https://en.wikipedia.org/wiki/Kea) (*Nestor Notabilis*), a curious and clever green parrot found in the Southern Alps of New Zealand. It was inspired by [OpenGrok](https://oracle.github.io/opengrok/).

**Goals:**
* Support basic LSP features (goto definition, find references, autocompletion)
* Basic scope awareness
* Efficient for large codebases
* Single rust binary

**Non-goals:**
* Type-aware analysis
* Preprocessor handling beyond goto definition for macros
* Full unicode support (will probably work to some extent, but there will likely be normalisation and potential perf issues)
* Support for text encodings other than UTF-8

## Usage

This project is very much a work-in-progress. To use it, build the `nestor` binary using Cargo then configure your editor to use it as a stdio language server.

* For Zed: https://github.com/zed-industries/zed/discussions/24092#discussioncomment-15278796
