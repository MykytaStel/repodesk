import { onboardedFixtures } from "./fixtures";
import type { CommandFixtures } from "./fixtures";

const engineeringSnapshot = onboardedFixtures.work_engineering_intelligence as Record<string, unknown>;

/**
 * Current-product overrides for the long-lived onboarded fixture.
 *
 * Keep the base fixture useful for legacy/advanced surfaces while this layer
 * tracks the canonical Work Strategy, Engineering Knowledge, Runs and Settings
 * read models. That makes contract migrations explicit instead of teaching the
 * mock IPC transport to silently translate deprecated commands.
 */
export const currentOnboardedFixtures: CommandFixtures = {
  ...onboardedFixtures,
  provider_preferences: {
    ollama_enabled: true,
    ollama_url: "http://127.0.0.1:11434",
    ollama_model: "qwen2.5-coder:7b",
    lm_studio_enabled: false,
    lm_studio_url: "http://127.0.0.1:1234",
    llamafile_enabled: false,
    llamafile_url: "http://127.0.0.1:8080",
    localai_enabled: false,
    localai_url: "http://127.0.0.1:8080",
    chatgpt_enabled: true,
    codex_enabled: true,
    gemini_enabled: false,
    openai_api_enabled: false,
    openai_api_key_env_var: "OPENAI_API_KEY",
    gemini_api_enabled: false,
    gemini_api_key_env_var: "GEMINI_API_KEY",
    anthropic_api_enabled: false,
    allow_paid_agents: false,
    codex_quota_status: "available",
    preferred_patch_provider: "codex_cli",
    preferred_compression_provider: "ollama",
    preferred_review_provider: "codex_cli",
    notes: "",
  },
  credential_status: [
    { key: "openai_api_key", configured: false, hint: "", source: "none" },
    { key: "anthropic_api_key", configured: false, hint: "", source: "none" },
    { key: "gemini_api_key", configured: false, hint: "", source: "none" },
  ],
  work_strategy_execution_preview: {
    execution: {
      goal: "Wire N2 E2E smoke",
      steps: [
        {
          step_id: "implement",
          title: "Implement",
          executor_label: "Codex CLI",
          executor_kind: "coding_agent",
          model: "codex",
          allow_write: true,
          isolated_workspace: true,
          paid: false,
          estimated_input_tokens: 4_800,
          estimated_output_tokens: 1_200,
          estimated_cost_units: 0,
        },
      ],
      context: {
        prepared: true,
        context_tokens: 4_200,
        candidate_tokens: 7_100,
        token_budget: 8_000,
        included_sources: 6,
        excluded_sources: 4,
        context_fingerprint: "context-fixture-8f124fc0872f",
        generated_at: "2026-08-12T18:30:00Z",
        warning: null,
      },
      total_estimated_tokens: 6_000,
      total_estimated_cost_units: 0,
      currency_label: "cost_units",
      expected_writes: true,
      isolated_workspace: true,
      requires_coding_agent_approval: true,
      requires_paid_approval: false,
    },
    strategy: {
      requested_mode: "auto",
      profile: "lean",
      plan_shape: "single_writer",
      economy_mode: "balanced",
      reuse_prepared_context: true,
      max_agent_steps: 1,
      independent_ai_review: false,
      feedback_influenced: false,
      feedback_detail: null,
      reasons: [
        {
          code: "narrow_scope",
          detail: "The fixture represents a bounded implementation task, so Auto can use one isolated writer.",
        },
      ],
    },
    comparison: {
      baseline_steps: 3,
      planned_steps: 1,
      baseline_estimated_tokens: 12_400,
      planned_estimated_tokens: 6_000,
      estimated_saved_tokens: 6_400,
      baseline_estimated_cost_units: 0,
      planned_estimated_cost_units: 0,
      estimated_cost_delta_units: 0,
    },
    plan_fingerprint: "plan-fixture-43e9bc904a4d",
  },
  work_engineering_intelligence: {
    ...engineeringSnapshot,
    knowledge_lifecycle: {
      project: "RepoDesk",
      generated_at: "2026-08-12T18:30:00Z",
      counts: {
        pending_review: 0,
        current: 0,
        review_soon: 0,
        review_required: 0,
        archived: 0,
      },
      entries: [],
    },
    run_observability: {
      run_id: "run-20260616-101500",
      disposition: {
        state: "ready",
        stage: "commit",
        code: "ready_to_commit",
        title: "Ready to commit",
        detail: "The reviewed ChangeSet is verified and ready for the bounded commit step.",
      },
      strategy: null,
      context: {
        candidate_tokens: 7_100,
        included_tokens: 4_200,
        compacted_tokens: 2_900,
        compactness_ratio: 0.5915,
        repeated_tokens: 756,
        repeated_context_ratio: 0.18,
      },
      efficiency: {
        workers: 1,
        successful_workers: 1,
        failed_workers: 0,
        blocked_workers: 0,
        skipped_workers: 0,
        handoffs: 0,
        unique_providers: 1,
        unique_models: 1,
        total_tokens: 5_100,
        tokens_per_changed_file: 2_550,
        cost_per_changed_file: 0,
        input_output_ratio: 4.6667,
      },
    },
  },
};
