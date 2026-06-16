// Real-backend first-run smoke. With REPODESK_HOME pointed at a throwaway dir,
// the app has no project/task, so the workflow surface must show onboarding.
// This exercises the full stack — WebView frontend, Tauri IPC, repodesk-core —
// not a mock (cf. ../e2e/first-run.spec.ts which mocks IPC).

describe("RepoDesk desktop — first-run smoke", () => {
  it("boots the app shell", async () => {
    await $(".app-shell").waitForExist({ timeout: 30_000 });
    await expect($(".brand")).toBeDisplayed();
    await expect($("nav.nav-list")).toBeExisting();
  });

  it("renders the daily-loop navigation", async () => {
    await expect($("//nav//button//strong[text()='Workflow']")).toBeExisting();
    await expect($("//nav//button//strong[text()='Settings']")).toBeExisting();
  });

  it("funnels into onboarding against a throwaway REPODESK_HOME", async () => {
    const onboarding = $("//*[contains(text(), 'Connect a project')]");
    await onboarding.waitForExist({ timeout: 30_000 });
    await expect(onboarding).toBeDisplayed();
    // No active project until one is connected.
    await expect($("//h2[contains(text(), 'No active project')]")).toBeExisting();
  });
});
