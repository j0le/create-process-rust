# create-process-rust

`create-process-rust` is a command line utility for Microsoft Windows to explore the strange behaviour of command lines on Windows compared to UNIX.

## Command lines on Windows

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
- UNIX paths are converted to Windows paths (forward slash to backslash)
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

## Usage

Use `--help` to get the up-to-date usage description:

```
create-process-rust.exe --help
```

Here is an example. If we enter this commandline in git-bash:

```bash
target/debug/create-process-rust.exe --print-args-only 'Alice asks: "How are you?"'      'Bob answers: "I'\''m fine!"' --some-path /c/Program\ Files/Git
```

We get for example this output:

```
The command line was converted losslessly.
The command line is put in quotes (»«). If those quotes are inside the command line, they are not escaped. The command line is:
»C:\Users\j0le\prog\create-process-rust\target\debug\create-process-rust.exe --print-args-only "Alice asks: \"How are you?\"" "Bob answers: \"I'm fine!\"" --some-path "C:/Program Files/Git"«

Argument  0,   0 ..  75, lossless: »C:\Users\j0le\prog\create-process-rust\target\debug\create-process-rust.exe«, raw: »C:\Users\j0le\prog\create-process-rust\target\debug\create-process-rust.exe«
Argument  1,  76 ..  93, lossless: »--print-args-only«, raw: »--print-args-only«
Argument  2,  94 .. 124, lossless: »Alice asks: "How are you?"«, raw: »"Alice asks: \"How are you?\""«
Argument  3, 125 .. 153, lossless: »Bob answers: "I'm fine!"«, raw: »"Bob answers: \"I'm fine!\""«
Argument  4, 154 .. 165, lossless: »--some-path«, raw: »--some-path«
Argument  5, 166 .. 188, lossless: »C:/Program Files/Git«, raw: »"C:/Program Files/Git"«
```

If we enter the following into `cmd.exe`:

```
target\debug\create-process-rust.exe --print-args-only "Alice asks: \"How are you?\""      "Bob answers: \"I'm fine!\"" --some-path "C:/Program Files/Git"
```

We get:

```
The command line was converted losslessly.
The command line is put in quotes (»«). If those quotes are inside the command line, they are not escaped. The command line is:
»target\debug\create-process-rust.exe  --print-args-only "Alice asks: \"How are you?\""      "Bob answers: \"I'm fine!\"" --some-path "C:/Program Files/Git"«

Argument  0,   0 ..  36, lossless: »target\debug\create-process-rust.exe«, raw: »target\debug\create-process-rust.exe«
Argument  1,  38 ..  55, lossless: »--print-args-only«, raw: »--print-args-only«
Argument  2,  56 ..  86, lossless: »Alice asks: "How are you?"«, raw: »"Alice asks: \"How are you?\""«
Argument  3,  92 .. 120, lossless: »Bob answers: "I'm fine!"«, raw: »"Bob answers: \"I'm fine!\""«
Argument  4, 121 .. 132, lossless: »--some-path«, raw: »--some-path«
Argument  5, 133 .. 155, lossless: »C:/Program Files/Git«, raw: »"C:/Program Files/Git"«
```

As you can see, cmd.exe preserves spaces between arguments, and git-bash does not. But somehow an extra spaces apears after argument zero.

### Tip for git-bash / MSYS2 bash

Set the environement variable `MSYS_NO_PATHCONV` to `1` to disable path conversion; for example:

```sh
MSYS_NO_PATHCONV=1 ./create-process-rust.exe --program "$(cygpath -wa "$(which cmd.exe)" )" --cmd-line-in-arg '/c (echo hello world)'
```

## Build Instructions for Windows

Install git, by taking one of these links:
- https://git-scm.com/download/win
- https://gitforwindows.org/


Install the rust toolchain with rustup by following these instructions: https://www.rust-lang.org/tools/install

Clone this repository:

```
cd some/nice/directory
git clone https://github.com/j0le/create-process-rust.git
```

Replace `some/nice/directory` with the path to some nice directory.

Change your current working directory to the directory, where the repository is:

```
cd create-process-rust
```

If you use cmd.exe as your shell, execute this:

```
"%USERPROFILE%\.cargo\bin\cargo.exe" build
```

If you use git-bash, execute this:

```
~/.cargo/bin/cargo build
```

To execute the program, use this command.

```
cargo run -- --help
```

Replace `cargo` with the path to the cargo executable.

## License

For lincense information see the file `LICENSE.txt`.
