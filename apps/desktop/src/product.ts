export type RiskLevel = "safe" | "guarded" | "expensive" | "blocked";

export type ProductArea = {
  id: string;
  title: string;
  subtitle: string;
  description: string;
  signal: string;
};

export const PRODUCT_AREAS: ProductArea[] = [
  {
    id: "brain",
    title: "Brain",
    subtitle: "RepoDesk decides what should happen next.",
    description:
      "The brain reads project/task state, checks context, judges risk, and suggests a next action instead of blindly calling agents.",
    signal: "workflow / doctor / judge",
  },
  {
    id: "management",
    title: "Management",
    subtitle: "Projects and tasks become first-class objects.",
    description:
      "The desktop app should let you select a project, create a task, and then build context around that task without touching terminal every time.",
    signal: "project list / project use / task new",
  },
  {
    id: "context",
    title: "Context",
    subtitle: "Small, task-focused packs instead of dumping the whole repo.",
    description:
      "Context generation is the token-control layer. Smart context should include only active task, changed files, repo map, and check summaries.",
    signal: "context.md / smart-context.md / token estimate",
  },
  {
    id: "security",
    title: "Security",
    subtitle: "Every dangerous action needs a gate.",
    description:
      "UI actions are whitelisted. No unrestricted shell access is exposed to the desktop interface. Agent access must go through guard/judge.",
    signal: "sandbox / safety scan / access matrix",
  },
  {
    id: "runtime",
    title: "Runtime",
    subtitle: "AI systems are modules, not the core itself.",
    description:
      "Ollama, ChatGPT, Codex, Gemini, shell, and future MCP adapters are peripherals. RepoDesk routes work to them based on need and risk.",
    signal: "runtime providers / route need",
  },
];

export const WORKFLOW_STEPS = [
  {
    id: "select",
    title: "1. Select project and task",
    body: "Choose the active project and create or inspect the active task. This prevents agents from working with vague context.",
  },
  {
    id: "understand",
    title: "2. Understand current state",
    body: "Read dashboard, workflow doctor, active project, active task, and git state before doing any work.",
  },
  {
    id: "context",
    title: "3. Build bounded context",
    body: "Generate context and smart context. This is what agents should see instead of the full repository.",
  },
  {
    id: "safety",
    title: "4. Scan for risky content",
    body: "Run safety scan before sending context to any AI system. Secrets and private files must not leak.",
  },
  {
    id: "judge",
    title: "5. Judge the agent",
    body: "Ask the judge if Codex/ChatGPT/Ollama should be allowed, warned, or blocked for the current task.",
  },
  {
    id: "act",
    title: "6. Execute a small bounded action",
    body: "Use a whitelisted action. Avoid unrestricted commands from UI until policy is stronger.",
  },
  {
    id: "verify",
    title: "7. Verify and record",
    body: "Run checks, inspect output, and keep action history so the system can explain what happened.",
  },
];
