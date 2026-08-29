# Browser testing with TestBox

Real browser tests for a JavaScript-driven form, in the test framework you
already use. Fill it in, submit it, assert on what appears.

```
signup.cfm            the app under test — a form whose output is built by JS
specs/SignupSpec.cfc  the tests
runner.cfm            TestBox runner (HTML in a browser, ?reporter=text for CI)
run.sh                start a server, run the specs, exit non-zero on failure
```

```sh
./run.sh                                   # CI shape
# or, to watch it in a browser:
OBSCURA_ALLOW_PRIVATE_NETWORK=1 rustcfml --serve
open http://127.0.0.1:8500/examples/05-testbox/runner.cfm
```

```
( √ ) the signup form
    ( √ ) shows nothing until it is submitted (11 ms)
    ( √ ) refuses an empty form and says why (12 ms)
    ( √ ) rejects an address that is not an email (12 ms)
    ( √ ) creates the account and echoes back what was entered (132 ms)
    ( √ ) prices each plan correctly (415 ms)
    ( √ ) actually painted something (174 ms)

[Passed: 6] [Failed: 0] [Errors: 0]     Duration: 765 ms
```

## Why this needs a browser

Every assertion is about markup that **does not exist in the HTML the server
sends**. The summary panel, the prices, the inline validation errors are all
built by JavaScript after the click. `cfhttp` fetches the empty form and can
say nothing about any of it.

## What the spec shows

- **`waitForSelector` rather than sleeping.** The summary is rendered
  asynchronously. Waiting for the element is both faster and not flaky.
- **Fluent calls chain**, so a spec reads like the interaction it describes:
  `page.fill(...).selectOption(...).click(...).waitForSelector(...)`.
- **One browser for the suite, a fresh page load per spec.** Starting a browser
  is the expensive part; `beforeEach` re-navigates so specs stay independent.
- **Assert that something was painted.** A page whose script died before
  painting still produces a perfectly valid, perfectly *blank* PNG, and a test
  that only checks the DOM will not notice. `screenshot( { detail = true } )`
  reports ink coverage; the last spec fails on a blank render and writes
  `failed.png` as evidence.

## They actually fail

A green suite means nothing until you have watched it go red, so both of these
were checked while writing it:

| change to `signup.cfm` | result |
|---|---|
| `team: 29` → `team: 31` | `[Passed: 5] [Failed: 1]` — *team should cost $29. Expected [$29] Actual [$31]* |
| rename the `summary` test id | `[Passed: 3] [Errors: 3]` — the waits time out |

`run.sh` exits non-zero in both cases, and zero when they pass.

## Notes

- `Application.cfc` maps `/testbox` to `./testbox` if you have run
  `box install testbox`, otherwise to a checkout beside this repo. Edit it if
  yours lives elsewhere.
- The specs drive the browser at the server running them, so they need
  `OBSCURA_ALLOW_PRIVATE_NETWORK=1`: the renderer refuses private addresses by
  default, and testing your own app is exactly when you mean to allow it.
- `run.sh` refuses to start if its port is busy rather than talking to whatever
  is already there — a stray CommandBox server answered these requests once and
  reported the specs as "file not found" from a different webroot.
