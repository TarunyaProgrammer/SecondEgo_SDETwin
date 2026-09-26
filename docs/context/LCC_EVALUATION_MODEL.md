Yes. The key thing to understand is that **your GitHub repository is not the thing they directly test against a predefined input like a normal software submission**. They are evaluating your **running harness** on a repository + issue that they provide.

There are two different repositories/concepts involved:

1. **Your harness repository** — what you submit.
2. **The evaluation repository** — the codebase/issues that your harness will be asked to modify.

The official spec makes this distinction fairly clear. Every team gets the same evaluation repository, issues, and tests, while the harness architecture is yours.

---

# 1. What you actually submit

Your GitHub repository contains your **AI coding harness**.

Something conceptually like:

```text
your-harness/
│
├── Makefile
├── README.md
├── harness/
│   ├── agent.py
│   ├── orchestrator.py
│   ├── tools/
│   └── ...
│
├── telemetry/
├── reporting/
├── configuration/
└── ...
```

The important interface is:

```bash
export AI_API_KEY="..."

make setup
make run
```

The submission document explicitly says this is the expected evaluator workflow.

`make test` is also required as a Makefile command, although the document says the **minimum** that must work is `make setup` and `make run`.

---

# 2. What happens when they evaluate you

Think of the evaluator doing something approximately like this:

```text
             YOUR GITHUB REPOSITORY
                      │
                      ▼
                git clone
                      │
                      ▼
          export AI_API_KEY="..."
                      │
                      ▼
                  make setup
                      │
                      ▼
                  make run
                      │
                      ▼
        ┌─────────────────────────┐
        │     YOUR HARNESS        │
        │                         │
        │  orchestrator           │
        │  context manager        │
        │  tools                  │
        │  Gemini                 │
        │  memory/state           │
        │  verification           │
        └────────────┬────────────┘
                     │
                     ▼
          EVALUATION REPOSITORY
                     +
              ASSIGNED ISSUE
                     │
                     ▼
             YOUR HARNESS WORKS
                     │
             ┌───────┼────────┐
             ▼       ▼        ▼
           files   terminal   tests
             │       │        │
             └───────┼────────┘
                     ▼
             final repository
                     │
             ┌───────┼──────────┐
             ▼       ▼          ▼
           result  transcript  telemetry
```

The official live-evaluation flow is:

> receive repository + issue → write prompt → freeze prompt → harness executes autonomously → capture transcript/telemetry → evaluate tests/repository → technical questioning → score.

---

# 3. So where does the user's input come from?

This is the part you're asking about.

**Yes: your harness needs some way to receive the task.**

But the official documents do **not** specify the exact interface for how the evaluation issue is injected into your running program.

They explicitly say:

> "The prescribed GitHub issue/test case will then be supplied to the running harness according to the official evaluation procedure."



And the main specification says:

> "The team may write one user-level prompt instructing its harness to solve the issue."



So the exact mechanism — whether they type into your TUI, pass a command-line argument, paste the issue into stdin, etc. — **is not documented in the resources you gave me**.

I cannot determine that exact interface from the provided resources.

However, the conceptual behavior is unambiguous.

---

# 4. What your harness should look like during testing

You should build your harness so that **you can manually give it a task**.

For example, your local version could launch:

```bash
make run
```

and show:

```text
╔══════════════════════════════════════════════╗
║          AI CODING HARNESS                   ║
╠══════════════════════════════════════════════╣
║ Repository: ./target-repo                    ║
║ Model: Gemini ...                            ║
║ Status: READY                                ║
╚══════════════════════════════════════════════╝

Task:
>
```

Then you could type:

```text
Fix the authentication bug described in issue #42.
Inspect the repository, implement the fix, run the existing tests,
and verify the solution. Do not modify tests.
```

Then:

```text
> Fix authentication bug...
```

And your harness takes over.

---

# 5. Your second testing repository is exactly useful for this

Suppose you have:

```text
my-harness/
```

and separately:

```text
test-repository/
```

The second repository should behave like a **fake evaluation repository**.

For example:

```text
test-repository/
├── src/
├── tests/
├── package.json
├── README.md
└── ...
```

Then your harness needs to be able to operate on it.

You could have:

```bash
make run REPO=../test-repository
```

or:

```bash
python harness.py ../test-repository
```

or have your TUI ask:

```text
Repository:
> ../test-repository

Task:
> Fix issue #17...
```

**That exact local interface is your design choice.**

The competition itself only standardizes the evaluator's entry point (`make run`) and leaves your internal architecture open.

---

# 6. The important distinction: your harness does NOT receive a pre-written solution

Suppose their evaluation repository contains:

```text
repo/
├── src/
├── tests/
└── ...
```

and the issue is:

> "The `/users` endpoint returns 500 when the database contains a null email. Fix the bug and ensure existing behavior remains intact."

The evaluator gives your harness the issue.

Your harness should then autonomously do something like:

```text
USER PROMPT
    │
    ▼
Understand issue
    │
    ▼
Search repository
    │
    ▼
Inspect relevant files
    │
    ▼
Build hypothesis
    │
    ▼
Modify code
    │
    ▼
Run tests
    │
    ▼
Tests fail?
    │
 ┌──┴───┐
 YES    NO
 │       │
 ▼       ▼
Analyze  Verify
failure   │
 │        ▼
 ▼      Finish
Modify
again
 │
 ▼
Run tests again
```

This is literally the behavior the specification describes: understand → explore → plan → use tools → modify implementation → test → analyze failure → recover/revise → verify → complete.

---

# 7. And this is why your harness needs tools

Your model cannot just receive:

```text
Fix issue #42.
```

and magically edit the repository.

Your harness needs to give the model capabilities such as:

```text
list_files()
read_file()
search_code()
write_file()
edit_file()
run_command()
run_tests()
git_diff()
git_status()
```

Potentially more.

For example:

```text
Gemini
   │
   │ "I need to inspect auth.py"
   ▼
Harness tool router
   │
   ▼
read_file("auth.py")
   │
   ▼
actual filesystem
   │
   ▼
contents
   │
   ▼
Gemini
```

Then:

```text
Gemini
   │
   │ "Run the tests"
   ▼
run_command("pytest")
   │
   ▼
test output
   │
   ▼
Gemini
```

The evaluation specifically examines tool usefulness, interface design, reliability, error handling, selection, sequencing and recovery.

---

# 8. Your `make run` therefore shouldn't just run one model call

This would be a weak architecture:

```python
prompt = input()
response = gemini(prompt)
print(response)
```

That isn't really a coding harness.

Your `make run` should launch the **agent runtime**.

Something conceptually like:

```python
while not finished:

    task = state.task

    context = context_manager.build_context(state)

    response = model.generate(
        context=context,
        tools=tools
    )

    action = parse(response)

    result = execute(action)

    state.update(result)

    if failure(result):
        recovery_manager.handle(result)

    if verification_passed():
        finished = True
```

The specification explicitly says one-shot does **not** mean one model call. Your harness may perform multiple model calls, tool calls, planning cycles, test executions, retries and recovery steps.

That's a major point.

---

# 9. Your local testing should therefore simulate the real evaluation

I would structure your development environment like this:

```text
                    YOU
                     │
                     ▼
                make run
                     │
                     ▼
             YOUR HARNESS
                     │
          ┌──────────┴──────────┐
          ▼                     ▼
     test-repository         Gemini
          │                     │
          └──────────┬──────────┘
                     ▼
                Agent Loop
                     │
         ┌───────────┼───────────┐
         ▼           ▼           ▼
       Search       Edit       Terminal
         │           │           │
         └───────────┼───────────┘
                     ▼
                   Tests
                     │
              ┌──────┴──────┐
              ▼             ▼
            FAIL           PASS
              │             │
              ▼             ▼
           Recover       Verify
              │
              └──────► ...
```

Your **test repository should deliberately contain bugs/issues**.

Don't just test:

> "Create hello world."

Create actual SWE-style tasks.

For example:

```text
Issue #1
Fix incorrect pagination when page=0.

Issue #2
Fix race condition in cache invalidation.

Issue #3
Add missing validation for malformed input.

Issue #4
Fix regression introduced in authentication middleware.
```

And have existing tests that expose the problems.

That gives you something close to the actual evaluation environment.

---

# 10. You should test the complete lifecycle

For every test task, record:

### Before

```bash
git status
git diff
pytest
```

### Run harness

```bash
make run
```

Give it:

```text
Solve issue #17.
Inspect the repository.
Implement the required change.
Use existing tests for verification.
Do not modify tests.
Continue autonomously until the implementation is verified.
```

Then let it run **without helping it**.

This is important because the competition is one-shot. Once execution starts, you cannot manually intervene, modify the prompt, restart it, or give additional instructions.

### After

Check:

```bash
git diff
git status
pytest
```

And inspect:

```text
Did it actually solve the issue?
Did it modify unnecessary files?
Did it modify tests?
Did it recover from failures?
Did it waste huge amounts of context?
Did it stop too early?
Did it get stuck?
```

Those observations map directly to the judging criteria.

---

# 11. Your `make test` should NOT mean "pretend evaluation"

This is an important architectural point.

You could make:

```bash
make test
```

run your own harness regression suite.

For example:

```text
make test
   │
   ├── Test 1: repository search
   ├── Test 2: file modification
   ├── Test 3: test failure recovery
   ├── Test 4: context compression
   ├── Test 5: termination
   └── Test 6: full SWE task
```

That's useful during development.

But don't confuse:

```text
make test
```

with the **official evaluation**.

The official evaluation is the live one-shot task performed against the organizer-provided repository and issue.

---

# 12. What the judges actually see

This is where your architecture matters.

They aren't simply looking at:

```text
PASS
```

They collect several kinds of evidence:

### A. Final repository

Did your harness actually solve the issue?

### B. Tests

Did the implementation pass the provided tests?

### C. Transcript

They can see:

```text
User prompt
↓
Model call
↓
Tool call
↓
Tool result
↓
Model call
↓
Tool call
↓
Test
↓
Failure
↓
Recovery
↓
...
↓
Final answer
```

### D. Telemetry

They measure things such as:

```text
model calls
tokens
context size
tool calls
execution time
retries
errors
recovery
```

### E. Technical interview

They can then ask:

> Why did your orchestrator do this?

> Why did you send this context?

> What happens when tests fail?

> How does your harness know the task is complete?

> How do you prevent context from growing indefinitely?

These are explicitly listed in the specification.

---

# 13. This means you should NOT optimize only for "it works"

Your test repository should let you investigate all of these:

| What you test                          | Why                            |
| -------------------------------------- | ------------------------------ |
| Can it understand an issue?            | Task completion                |
| Can it find relevant files?            | Context/repository exploration |
| Can it edit correctly?                 | Correctness                    |
| Can it run tests?                      | Verification                   |
| Can it interpret failures?             | Recovery                       |
| Can it retry intelligently?            | Reliability                    |
| Can it avoid rereading everything?     | Context efficiency             |
| Can it terminate?                      | Orchestration                  |
| Can it avoid useless calls?            | Resource efficiency            |
| Can it preserve existing architecture? | Code quality                   |

The rubric gives **30/100 to task completion/correctness**, followed by orchestration and context management at 15 each.

So don't build some elaborate multi-agent architecture that is unreliable on actual coding tasks.

---

# 14. One thing in the documents you should be aware of

There is a small inconsistency between the supplied materials.

The formal specification currently says:

> Results approximately **1–2 days** after evaluation.

The presentation document says:

> Results approximately **7–10 days** after evaluation.

The formal specification also labels itself **Draft**.

That doesn't affect how you build/test the harness, but it means you shouldn't treat every operational detail in these documents as permanently finalized.

More importantly, the formal spec says some details such as **exact runtime/API configuration/infrastructure** will be finalized before the event, and the external-model rule is still marked TBD.

---

# 15. What I would build for your second testing repository

Your immediate setup should be:

```text
AI-Harness/
│
├── Makefile
├── harness/
│   ├── main.py
│   ├── orchestrator.py
│   ├── model.py
│   ├── context.py
│   ├── state.py
│   ├── verifier.py
│   └── tools/
│       ├── filesystem.py
│       ├── search.py
│       ├── terminal.py
│       └── git.py
│
└── tests/
    └── harness_tests/
```

Then separately:

```text
Harness-Test-Repo/
│
├── src/
├── tests/
├── README.md
└── ...
```

And your local invocation should be something like:

```bash
export AI_API_KEY="your-key"

make setup

make run
```

Then:

```text
Repository: ../Harness-Test-Repo

Task:
Fix issue #3. Investigate the repository, implement the fix,
run the existing tests, and verify the final implementation.
Do not modify tests.
```

**From that point onward, you should not touch the repository yourself.**

Watch your harness operate.

That's your miniature version of the actual competition.

---

## The simplest mental model

Don't think:

> "I submit a GitHub repo and judges run `make test`."

Think:

> **"I submit an executable AI engineer."**

The evaluator essentially does:

```text
clone my harness
       ↓
make setup
       ↓
make run
       ↓
give it an unfamiliar software repository + issue
       ↓
let my AI engineer work
       ↓
watch everything it does
       ↓
check whether it actually fixed the issue
       ↓
inspect how efficiently/reliably it worked
       ↓
ask me why I engineered it that way
```

That is the competition.

Your **second testing repository should therefore be treated as a miniature hostile evaluation environment**, not merely as a place to test whether your Gemini API call works.

The most important thing you should implement next is a **clean local evaluation harness** where you can point your agent at `./test-repo`, give it one issue, and then let it operate completely autonomously while recording the exact transcript, tool calls, tests, retries, and final diff. That will expose whether your architecture actually works before you face the one-shot evaluation.
