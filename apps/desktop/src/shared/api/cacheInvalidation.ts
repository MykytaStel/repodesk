import type { QueryClient, QueryKey } from "@tanstack/react-query";

export type QueryDomain = "workspace" | "work" | "git" | "code" | "runs" | "providers" | "system";

const DOMAIN_KEYS: Record<QueryDomain, readonly QueryKey[]> = {
  workspace: [
    ["desktop_snapshot"],
    ["get_active_project_config"],
    ["project_list_configs"],
    ["db_status"],
  ],
  work: [
    ["work"],
    ["product_workflow_state"],
    ["desktop_actions"],
    ["action_history"],
  ],
  git: [["git_workspace_snapshot"]],
  code: [
    ["code"],
    ["code_workbench_snapshot"],
  ],
  runs: [
    ["runs"],
    ["orchestration_runs"],
    ["orchestrate_status"],
    ["task_timeline"],
  ],
  providers: [
    ["provider_settings"],
    ["model_health_snapshot"],
    ["routing_snapshot"],
    ["token_usage_snapshot"],
    ["token_cost_trend"],
  ],
  system: [
    ["get_system_agents"],
    ["get_system_capabilities"],
    ["get_system_peripherals"],
    ["get_system_modules"],
  ],
};

/**
 * Invalidate only the query families whose source-of-truth may have changed.
 * This is intentionally prefix-based: e.g. ["work"] refreshes phase,
 * engineering, contract and related Work Item projections without waking model
 * health, token ledgers or system discovery.
 */
export async function invalidateQueryDomains(
  queryClient: QueryClient,
  domains: readonly QueryDomain[],
): Promise<void> {
  const seen = new Set<string>();
  const keys: QueryKey[] = [];

  for (const domain of domains) {
    for (const queryKey of DOMAIN_KEYS[domain]) {
      const signature = JSON.stringify(queryKey);
      if (seen.has(signature)) continue;
      seen.add(signature);
      keys.push(queryKey);
    }
  }

  await Promise.all(keys.map((queryKey) => queryClient.invalidateQueries({ queryKey })));
}
