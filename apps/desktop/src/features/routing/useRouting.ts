import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { queryKeys, optionalCommand } from "../../shared/api/queries";
import { RoutingSnapshot, ProviderSettings } from "../../shared/types/api";

export function useRouting(economyMode: string) {
  const routing = useQuery({
    queryKey: queryKeys.routing.snapshot(economyMode),
    queryFn: () => invoke<RoutingSnapshot>("routing_snapshot", { economyMode }).catch(() => null),
  });

  const settings = useQuery({
    queryKey: queryKeys.routing.settings,
    queryFn: () => optionalCommand<ProviderSettings>("provider_settings"),
  });

  const apiEnv = useQuery({
    queryKey: queryKeys.routing.apiEnv,
    queryFn: () => optionalCommand<any>("get_api_env_diagnostic"),
  });

  return {
    routing: routing.data,
    providerSettings: settings.data,
    apiEnvDiagnostic: apiEnv.data,
    isLoading: routing.isLoading || settings.isLoading,
  };
}
