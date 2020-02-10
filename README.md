![Crates.io](https://img.shields.io/crates/v/gkosgrep)
![Crates.io](https://img.shields.io/crates/d/gkosgrep)
![Crates.io](https://img.shields.io/crates/l/gkosgrep)

# Overview

Small tool for greping supporting .gitignore and custom .ignore file. This
make easy to ignore files that reside in git like rspec support files.

# Installation

    cargo install gkosgrep

# Usage

I did this to use inside vim as a replacement for [the silver searcher](https://github.com/ggreer/the_silver_searcher)
that would be easier to customize what is ignored. The usage is damn simple

    gkosgrep <pattern> <rootfolder=.> <ignorefiles=.gitignore .ignore>

I'm using `=` specify the defaults here. If ignored file is missing it is just
ignored. To use this with [fzf](https://github.com/junegunn/fzf.vim) inside vim, do:

    command! -bang -nargs=* Rg
      \ call fzf#vim#grep(
      \   '<REPLACE_PATH_HERE>/rgrep/target/release/rgrep . '.shellescape(<q-args>), 0,
      \   {}, <bang>0)

Then use `Rg` command to filter files
