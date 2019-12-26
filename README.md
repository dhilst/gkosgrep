A grep like tool written in Rust, a study case

I really loved Rust, it make the threading stuff simple. The borrowing
system force me to place regex compilation inside worker thread, but yeah
I getting used to it


To build `cargo build`

To run `cargo run <PATH> <PATTERN>`

It will search for files in `PATH` and filter it by `PATTERN`. If
`PATTERN` is omitted `.*` is used (which matches everything).

To use this in vim with fzf tool, place this in your .vimrc 

```
command! -bang -nargs=* Rg
  \ call fzf#vim#grep(
  \   '<REPLACE_PATH_HERE>/rgrep/target/release/rgrep . '.shellescape(<q-args>), 0,
  \   {}, <bang>0)
"
```
