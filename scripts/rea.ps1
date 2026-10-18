# Optional development tooling, kept separate from the Rust server dependencies.
# A supported Node runtime is selected locally by npm; the system installation is unchanged.
$ErrorActionPreference = 'Stop'
& npx --yes --package=node@24 --package=rea-agents@6.7.0 rea @args
exit $LASTEXITCODE
