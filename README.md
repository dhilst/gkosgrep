![Crates.io](https://img.shields.io/crates/v/gkosgrep)

# Overview

Small tool for greping supporting .gitignore and custom .ignore file. This
make easy to ignore files that reside in git like rspec support files.

# Installation

    cargo install gkosgrep

# Usage

I did this to use inside vim as a replacement for [the silver searcher](https://github.com/ggreer/the_silver_searcher)
that would be easier to customize what is ignored. The usage is damn simple

    gkosgrep <path> [pattern]

If ignored file is missing it is just ignored. To use this with
[fzf](https://github.com/junegunn/fzf.vim) inside vim, do:

    command! -bang -nargs=* Gkosgrep
          \ call fzf#vim#grep(
          \   $HOME.'/.cargo/bin/gkosgrep . '.shellescape(<q-args>), 0,
          \   {}, <bang>0)

Then use `Gkosgrep` command to filter files

If you (like me) want searches to ignore file names use this


    function! GkosGrepFzf(query, fullscreen)
      let command_fmt = 'gkosgrep . %s || true'
      let initial_command = printf(command_fmt, shellescape(a:query))
      let reload_command = printf(command_fmt, '{q}')
      let spec = {'options': ['--phony', '--query', a:query, '--bind', 'change:reload:'.reload_command]}
      call fzf#vim#grep(initial_command, 0, fzf#vim#with_preview(spec), a:fullscreen)
    endfunction
    command! -nargs=* -bang GkosGrep call GkosGrepFzf(<q-args>, <bang>0)
