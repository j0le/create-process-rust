param(
	[Parameter(Mandatory=$true)]
	[String]$Commandline
)
$local:ErrorActionPreference = 'Stop'


$cpr = Join-Path $PSScriptRoot '..' 'target/debug/create-process-rust.exe'

$cmdline_base64 = [System.Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Commandline))

#"Command line in base64: ${cmdline_base64}"

$json_text = & $cpr --json --split-and-print-inner-cmdline --dry-run --program-from-cmd-line --cmd-line-utf16le-base64 $cmdline_base64 2>$null

$json = $json_text | ConvertFrom-Json

$program = $json.args[0].arg

$program_path = (Get-Command -CommandType:Application -Name:$program).Source

$program_path_base64 = [System.Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($program_path))

& $cpr --split-and-print-inner-cmdline --program-utf16le-base64 $program_path_base64 --cmd-line-utf16le-base64 $cmdline_base64
