AI Harness Hackathon 2026
Standardised Makefile-Based Evaluation Setup
Participant Technical Requirements & Evaluation Guidelines
To ensure a uniform, reproducible, and technically consistent evaluation process, every participating team is required to include a standardised Makefile at the root of its submission repository.
The Makefile will provide the evaluation team with a consistent interface to configure, initialise, test, and execute your AI Harness.
1. Mandatory Makefile
Every submission must contain the following file at the root of the repository:
Makefile
The Makefile must expose the following commands:
make setup
make run
make test
Teams may implement these commands according to their own technology stack and architecture, but their purpose must remain consistent.
Command
Purpose
make setup
Install and configure all required dependencies
make run
Initialise and launch the AI Harness
make test
Execute the team's test/evaluation procedure
make clean
Remove generated artefacts, where applicable

Minimum Requirement
At minimum, the following must work successfully:
make setup
make run
2. API Key Configuration
The evaluation environment will provide the required API credential through an environment variable.
The variable name will be:
AI_API_KEY
You MUST NOT hard-code:
API keys
Access tokens
Passwords
Secrets
Authentication credentials
inside your:
Source code
Makefile
.env files
Documentation
Configuration files committed to Git
The evaluator will provide the credential at runtime:
export AI_API_KEY="<PROVIDED_API_KEY>"
Your application must read the credential from this environment variable.
For example:
setup:
	@echo "Setting up environment..."
	# installation commands
run:
	@echo "Starting AI Harness..."
	AI_API_KEY=$(AI_API_KEY) <team-run-command>
test:
	@echo "Running tests..."
	AI_API_KEY=$(AI_API_KEY) <team-test-command>
The exact implementation may differ depending on your technology stack.
3. Model Requirement
The AI Harness submitted for evaluation must use text-only language models.
The evaluation input will be provided as text.
Your implementation must not require:
Image input
Audio input
Video input
Multimodal processing
Any other non-text modality
This requirement ensures that all teams are evaluated under the same modality constraints.
4. Model Configuration
The model configuration used by your AI Harness must be clearly defined within the application or its configuration files.
If the Organising Committee specifies a particular model or model family for the official evaluation, teams must use the prescribed model.
Teams must not substitute another model during evaluation unless explicitly authorised by the Organising Committee.
The API credential will continue to be supplied externally through:
AI_API_KEY
Your implementation must therefore allow the credential to be supplied without modifying the submitted source code.
5. Standard Evaluation Procedure
The evaluation team will follow a common procedure for all submissions.
Step 1 — Obtain the Repository
The evaluator will obtain the team's submitted repository.
Step 2 — Configure API Credential
The evaluator will run:
export AI_API_KEY="<PROVIDED_API_KEY>"
Step 3 — Setup
The evaluator will run:
make setup
This command must install and configure everything required to run the project.
Step 4 — Launch
The evaluator will run:
make run
This must launch the team's AI Harness in its intended evaluation mode.
Step 5 — Evaluation
The prescribed GitHub issue/test case will then be supplied to the running harness according to the official evaluation procedure.
Where applicable, the evaluator may also run:
make test
6. TUI Requirements
Teams implementing a Terminal User Interface (TUI) must ensure that the TUI can be launched through:
make run
The evaluator should not have to discover team-specific commands to initialise your TUI.
The expected flow is:
Submitted Repository
        ↓
Configure AI_API_KEY
        ↓
make setup
        ↓
make run
        ↓
Standard Evaluation Environment
        ↓
Evaluation Issue / Test Case
        ↓
Harness Execution
        ↓
Result
Teams are free to determine:
TUI design
Navigation
Layout
Interaction model
Internal architecture
Visual presentation
However, the initialisation and execution interface must remain standardised.
7. Evaluation Environment
The Organising Committee will establish a standard evaluation environment.
The environment will provide, as applicable:
Prescribed runtime environment
Designated API credential
Prescribed text-only model
Relevant GitHub repository/issue
Required testing infrastructure
Standard execution proceed Absolutely — here is a participant-facing document version, cleaned up and formatted so you can directly send it to teams.
AI Harness Hackathon 2026
Standardised Makefile-Based Evaluation Setup
Participant Technical Requirements & Evaluation Guidelines
To ensure a uniform, reproducible, and technically consistent evaluation process, every participating team is required to include a standardised Makefile at the root of its submission repository.
The Makefile will provide the evaluation team with a consistent interface to configure, initialise, test, and execute your AI Harness.
1. Mandatory Makefile
Every submission must contain the following file at the root of the repository:
Makefile
The Makefile must expose the following commands:
make setup
make run
make test
Teams may implement these commands according to their own technology stack and architecture, but their purpose must remain consistent.
Command
Purpose
make setup
Install and configure all required dependencies
make run
Initialise and launch the AI Harness
make test
Execute the team's test/evaluation procedure
make clean
Remove generated artefacts, where applicable

Minimum Requirement
At minimum, the following must work successfully:
make setup
make run
2. API Key Configuration
The evaluation environment will provide the required API credential through an environment variable.
The variable name will be:
AI_API_KEY
You MUST NOT hard-code:
API keys
Access tokens
Passwords
Secrets
Authentication credentials
inside your:
Source code
Makefile
.env files
Documentation
Configuration files committed to Git
The evaluator will provide the credential at runtime:
export AI_API_KEY="<PROVIDED_API_KEY>"
Your application must read the credential from this environment variable.
For example:
setup:
	@echo "Setting up environment..."
	# installation commands
run:
	@echo "Starting AI Harness..."
	AI_API_KEY=$(AI_API_KEY) <team-run-command>
test:
	@echo "Running tests..."
	AI_API_KEY=$(AI_API_KEY) <team-test-command>
The exact implementation may differ depending on your technology stack.
3. Model Requirement
The AI Harness submitted for evaluation must use text-only language models.
The evaluation input will be provided as text.
Your implementation must not require:
Image input
Audio input
Video input
Multimodal processing
Any other non-text modality
This requirement ensures that all teams are evaluated under the same modality constraints.
4. Model Configuration
The model configuration used by your AI Harness must be clearly defined within the application or its configuration files.
If the Organising Committee specifies a particular model or model family for the official evaluation, teams must use the prescribed model.
Teams must not substitute another model during evaluation unless explicitly authorised by the Organising Committee.
The API credential will continue to be supplied externally through:
AI_API_KEY
Your implementation must therefore allow the credential to be supplied without modifying the submitted source code.
5. Standard Evaluation Procedure
The evaluation team will follow a common procedure for all submissions.
Step 1 — Obtain the Repository
The evaluator will obtain the team's submitted repository.
Step 2 — Configure API Credential
The evaluator will run:
export AI_API_KEY="<PROVIDED_API_KEY>"
Step 3 — Setup
The evaluator will run:
make setup
This command must install and configure everything required to run the project.
Step 4 — Launch
The evaluator will run:
make run
This must launch the team's AI Harness in its intended evaluation mode.
Step 5 — Evaluation
The prescribed GitHub issue/test case will then be supplied to the running harness according to the official evaluation procedure.
Where applicable, the evaluator may also run:
make test
6. TUI Requirements
Teams implementing a Terminal User Interface (TUI) must ensure that the TUI can be launched through:
make run
The evaluator should not have to discover team-specific commands to initialise your TUI.
The expected flow is:
Submitted Repository
        ↓
Configure AI_API_KEY
        ↓
make setup
        ↓
make run
        ↓
Standard Evaluation Environment
        ↓
Evaluation Issue / Test Case
        ↓
Harness Execution
        ↓
Result
Teams are free to determine:
TUI design
Navigation
Layout
Interaction model
Internal architecture
Visual presentation
However, the initialisation and execution interface must remain standardised.
7. Evaluation Environment
The Organising Committee will establish a standard evaluation environment.
The environment will provide, as applicable:
Prescribed runtime environment
Designated API credential
Prescribed text-only model
Relevant GitHub repository/issue
Required testing infrastructure
Standard execution procedure
The objective is to minimise differences caused by individual machines, terminal configurations, or local environments.
8. Credential Security
The following practices are strictly prohibited:
Hard-coded API keys
Committed credentials
Credentials embedded in source code
Credentials embedded in Makefiles
Credentials committed in .env files
Credentials included in documentation
If environment variables are required for local development, teams may include:
.env.example
For example:
AI_API_KEY=
Do not include an actual API credential.
9. Environment Independence
Your Makefile should encapsulate the setup procedure required to initialise your project.
The evaluator should not ordinarily be required to:
Manually install undocumented dependencies
Determine the correct runtime command
Modify source code
Modify dependency files
Manually alter configuration files
Enter credentials into source files
Contact the team for routine setup assistance
All dependencies required for execution should be properly declared and handled through the setup procedure.
10. Reproducible Execution
Teams should ensure, to the extent reasonably practicable, that their AI Harness behaves consistently under the same evaluation conditions.
If your implementation uses:
Randomness
Configurable parameters
Random seeds
Variable execution settings
that materially affect the output, the relevant configuration should be documented or controlled within the submitted project.
The evaluation environment will remain consistent across teams to the extent technically feasible.
11. Evaluation Independence
Evaluation will be conducted using the submitted repository and the standard evaluation environment.
The evaluation team will not modify your implementation merely to make it executable.
If a submission cannot be initialised using the prescribed Makefile interface and standard environment, the issue may be documented and handled according to the official Hackathon evaluation and scoring policy.
12. Standard Evaluator Workflow
The intended evaluation workflow is:
git clone <TEAM_REPOSITORY>
cd <TEAM_REPOSITORY>
export AI_API_KEY="<PROVIDED_API_KEY>"
make setup
make run
The evaluator will then proceed with the prescribed evaluation issue/test case.
Where automated testing is supported:
make test
may also be executed according to the evaluation protocol.
13. Final Submission Checklist
Before submitting your project, test it in a clean environment.
Your team must verify that:
make setup
runs successfully.
Then verify:
make run
successfully launches your AI Harness.
Also verify, where applicable:
make test
Your repository should contain:
your-project/
│
├── Makefile              ← REQUIRED
├── README.md
├── source-code/
├── configuration-files/
├── dependency-files/
└── ...
The Makefile must be located at the root of the repository.
14. Important
The Makefile is an essential component of your final submission.
Every team is responsible for ensuring that the submitted repository can be set up and launched using the standard interface.
The evaluator should be able to do:
export AI_API_KEY="<PROVIDED_API_KEY>"
make setup
make run
without modifying your source code or manually configuring your project.
Failure to provide a functional standardised setup and execution interface may affect the ability of the evaluation team to execute your submission and will be handled according to the official Hackathon evaluation policy.
Final Requirement
Before the submission deadline, please test your repository from a clean environment and confirm that:
make setup
make run
work successfully.
Your Makefile is part of the submission and will be used during evaluation.

