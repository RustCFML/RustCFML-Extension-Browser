# Examples

Four small programs, each showing something you cannot do with `cfhttp`.
Run them from this repo once the extension is installed.

| | what it shows | run |
|---|---|---|
| `01-scrape` | Scrape a page into CFML data — selectors, `evaluate()`, markdown for an LLM | `rustcfml examples/01-scrape/main.cfm` |
| `02-qa-login` | QA a login flow and prove it worked with a screenshot | serve mode, see below |
| `03-report-to-pdf` | Render a CFML-built HTML report to PDF and PNG | `rustcfml examples/03-report-to-pdf/main.cfm` |
| `04-visual-check` | Detect a blank render, sweep viewports, and diff against a layout baseline | `rustcfml examples/04-visual-check/main.cfm` |

`02-qa-login` drives the browser at the server running it, and the renderer
refuses private addresses unless you opt in:

```sh
OBSCURA_ALLOW_PRIVATE_NETWORK=1 rustcfml --serve
open http://127.0.0.1:8500/examples/02-qa-login/main.cfm
```

`04-visual-check` stores a baseline on its first run and compares on every run
after, so run it twice to see the interesting half. Generated `.png`/`.pdf`
output and the baseline are gitignored.
