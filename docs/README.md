# BattleTechMUX documentation site

This Hugo site uses the Docsy theme through a Hugo module. Install the Node
dependencies once for Docsy's PostCSS step, then build or serve it with:

```sh
npm --prefix docs install
just docsite
just docsite-serve
```

The generated `docs/public/` directory is not tracked.
