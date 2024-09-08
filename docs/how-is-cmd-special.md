# How is cmd.exe special?

We start cmd.exe with this command line:

```
"C:\Windows\System32\cmd.exe" /C ""C:\Program Files\Git\cmd\git.exe" --version"
```

We can do that by entering that exact command line into a cmd.exe prompt, or by running this in git-bash:

```bash
MSYS_NO_PATHCONV=1 target/debug/create-process-rust.exe --print-args --split-and-print-inner-cmdline --program 'C:\Windows\System32\cmd.exe' --prepend-program --cmd-line-in-arg '/C ""C:\Program Files\Git\cmd\git.exe" --version"'
```

In the second case, you will notice that `create-process-rust` prints this message:
```
The command line (2nd argument to CreateProcessW) is:   »C:\Windows\System32\cmd.exe /C ""C:\Program Files\Git\cmd\git.exe" --version"«
```

If cmd.exe would use the Microsoft CRT algorithm, we would expect that cmd.exe divides the command line in these arguments:
- `C:\Windows\System32\cmd.exe`
- `/C`
- `C:\Program`
- `Files\Git\cmd\git.exe --version`

But that is clearly not the case, because cmd.exe successfully starts git.exe.
Thus cmd.exe doesn’t use the usual algorithm.
