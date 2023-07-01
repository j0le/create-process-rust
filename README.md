# create-process-rust

`create-process-rust` is a command line utility for Microsoft Windows to explore the strange behaviour of command lines in Windows compared to UNIX.

On Windows, if a process asks the operating system for it's command line, it doesn't get an array of arguments, but only *one* UTF-16 string.
The parsing into individual arguments is normally done with this algorithm: https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments?view=msvc-170 .
This is the algorithm of the Microsoft C-Runtime. (It can also be implemented in other languages.)
But many programs including `cmd.exe` and `msbuild.exe` do it differently.

When working in a shell or command prompt, one has to think about the how the shell processes the input of the user and how it construct the command line, that it passes to [`CreateProcessW()`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw).
`CreateProcessW()` is the low level function to create processes on Windows.

For example the projects “[git for Windows](https://gitforwindows.org/)”, [MSYS2](https://www.msys2.org/) and [Cygwin](https://www.cygwin.com/) port the shell bash to windows.
This ported bash works like this (simplified):

- The user enters a command line
- The command line is split into an array of arguments, respecting the special characters of bash, for example: `'`, `"`, `$`, ...
- A new command line is put together acording to the [Microsoft CRT algorithm](https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments?view=msvc-170).
- `CreateProcessW()` is called with that command line.

By the way: PowerShell has very crazy rules how the final command line is created, and they are different between versions.
See for example:

- [PSNativeCommandArgumentPassing](https://learn.microsoft.com/en-us/powershell/scripting/learn/experimental-features?view=powershell-7.3#psnativecommandargumentpassing)
- Issue on GitHub: https://github.com/PowerShell/PowerShell/issues/14747

On Linux, if a process asks the operating system for it's command line, it gets an array of arguments directly from the OS.
These arguments do not have an encoding, but cannot contain a null byte. The encoding used to decode these arguments is typlically UTF-8.
But some programs look into environment variables to figure out, which arguments to use. 
I assume, that those programs asume, that the environments variables (their names and their values) are encoded in ASCII.

## License

For lincense information see the file `LICENSE.txt`.
