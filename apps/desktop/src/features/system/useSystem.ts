import { useQuery } from "@tanstack/react-query";
import { queryKeys, optionalCommand } from "../../shared/api/queries";
import { AgentsConfig, CapabilitiesConfig, PeripheralsConfig, BrainModule } from "../../shared/types/api";

export function useSystem() {
  const agents = useQuery({
    queryKey: queryKeys.system.agents,
    queryFn: () => optionalCommand<AgentsConfig>("get_system_agents"),
  });

  const capabilities = useQuery({
    queryKey: queryKeys.system.capabilities,
    queryFn: () => optionalCommand<CapabilitiesConfig>("get_system_capabilities"),
  });

  const peripherals = useQuery({
    queryKey: queryKeys.system.peripherals,
    queryFn: () => optionalCommand<PeripheralsConfig>("get_system_peripherals"),
  });

  const modules = useQuery({
    queryKey: queryKeys.system.modules,
    queryFn: () => optionalCommand<BrainModule[]>("get_system_modules").then(res => res ?? []),
  });

  return {
    systemAgents: agents.data,
    systemCapabilities: capabilities.data,
    systemPeripherals: peripherals.data,
    systemModules: modules.data ?? [],
    isLoading: agents.isLoading || capabilities.isLoading || peripherals.isLoading || modules.isLoading,
  };
}
