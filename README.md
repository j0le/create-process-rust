# create-process-rust

`create-process-rust` is a command line utility for Microsoft Windows to explore the strange behaviour of command lines on Windows compared to UNIX.

## Command lines on Windows

A more in-depth explanation can be found here: https://daviddeley.com/autohotkey/parameters/parameters.htm#WIN .

On Windows, if a process asks the operating system for it's command line, it doesn't get an array of arguments, but only *one* UTF-16 string.
The parsing into individual arguments is normally done with this algorithm: https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments?view=msvc-170 .
This is the algorithm of the Microsoft C-Runtime. (It can also be implemented in other languages.)
But many programs parse their command line differently (for example `msbuild.exe` and [`cmd.exe`](/docs/how-is-cmd-special.md)).

When working in a shell or command prompt, one has to think about the how the shell processes the input of the user and how it construct the command line, that it passes to [`CreateProcessW()`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw).
`CreateProcessW()` is the low level function to create processes on Windows.

For example the projects “[git for Windows](https://gitforwindows.org/)”, [MSYS2](https://www.msys2.org/) and [Cygwin](https://www.cygwin.com/) port the shell [Bash](https://en.wikipedia.org/wiki/Bash_(Unix_shell)) to Windows.
This ported bash works like this (simplified):

- The user enters a command line
- The command line is split into an array of arguments, respecting the special characters of bash, for example: `'`, `"`, `$`, ...
- UNIX paths are converted to Windows paths
- A new command line is put together acording to the [Microsoft CRT algorithm](https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments?view=msvc-170).
- `CreateProcessW()` is called with that command line.

By the way: PowerShell has very crazy rules how the final command line is created, and they are different between versions.
See for example:

- [PSNativeCommandArgumentPassing](https://learn.microsoft.com/en-us/powershell/scripting/learn/experimental-features?view=powershell-7.3#psnativecommandargumentpassing)
- Issue on GitHub: https://github.com/PowerShell/PowerShell/issues/14747

On Linux, if a process asks the operating system for it's command line, it gets an *array* of arguments directly from the OS.
These arguments do not have an encoding, but cannot contain a null byte. The encoding used to decode these arguments is typlically UTF-8.
But some programs look into environment variables to figure out, which arguments to use. 
I assume, that those programs asume, that the environments variables (their names and their values) are encoded in ASCII.

## Different command lines &#x2013; same result

If a program named `prog` is excuted with any of these command lines and the algorithm of the Microsoft CRT is used, the result is always the same:

- `prog.exe --input:"Hello World"    --another-option`
- `prog.exe "--input:Hello World" --another-option`
- `prog.exe    --input:Hello" "World              --another-option`

The result is an array of these arguments:
- `prog.exe`
- `--input:Hello World`
- `another-option`

## Usage

Use `--help` to get the up-to-date usage description:

```
create-process-rust.exe --help
```

## Examples

### Example 1

Run each of these command lines one by one in git-bash and compare the output:

```bash
target/debug/create-process-rust.exe --print-args-only --input:"Hello World"    --another-option
target/debug/create-process-rust.exe --print-args-only --input:'Hello World'    --another-option
target/debug/create-process-rust.exe --print-args-only --input:Hello World" --another-option
target/debug/create-process-rust.exe --print-args-only   --input:Hello" "World              --another-option
target/debug/create-process-rust.exe --print-args-only --input:Hello\ World  --another-option

```

Also run them in `cmd.exe`.

### Example 2

Here is an example. If we enter this commandline in git-bash:

```bash
target/debug/create-process-rust.exe --print-args-only 'Alice asks: "How are you?"'      'Bob answers: "I'\''m fine!"' --some-path /c/Program\ Files/Git
```

We get this output:

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

As you can see, the command line is devided into six arguments (Arguments 0 through 5).
Notice these things:
- The program "create-process-rust" reports another command line than the one entered in the shell.
- Most single quotes (`'`) have become double quotes (`"`).
- Double quotes inside of arguments are preceded by backslashes.
- Multiple space characters are reduced to one.
- The UNIX-style path `/c/Program\ Files/Git` is converted to the Windows-style path `C:/Program Files/Git`.

All theses thins are done by git-bash.

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

#### Tip for git-bash / MSYS2 bash

Set the environement variable `MSYS_NO_PATHCONV` to `1` to disable path conversion; for example:

```sh
MSYS_NO_PATHCONV=1 ./create-process-rust.exe --program "$(cygpath -wa "$(which cmd.exe)" )" --cmd-line-in-arg '/c (echo hello world)'
```

### Example 3

To see how `cmd.exe` parses it’s command line differently see [docs/how-is-cmd-special.md](/docs/how-is-cmd-special.md).

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


## Related projects

by me:
- https://github.com/j0le/process-starter
- https://github.com/j0le/get-command-line
- https://github.com/j0le/cpwd – cpwd – Copy the path of the current working directory/folder to the clipboard

by others:
- https://github.com/mklement0/Native
- https://github.com/gerardog/gsudo
- https://github.com/yozhgoor/CreateProcessW (didn’t tested that yet)
- https://github.com/clap-rs/clap

## License

For lincense information see the file [LICENSE.txt](LICENSE.txt).
