// Real-backend first-run smoke. With REPODESK_HOME pointed at a throwaway dir,
// the app has no project/task, so the Work surface must stop at Scope and offer
// exactly one onboarding action. This exercises the full stack — WebView
// frontend, Tauri IPC, and repodesk-core — not mocked IPC.

describe("RepoDesk desktop — first-run smoke", () => {
  it("boots the current IDE shell", async () => {
    await $(".ide-shell").waitForExist({ timeout: 30_000 });
    await expect($(".activity-brand")).toBeDisplayed();
    await expect($(".activity-rail")).toBeExisting();
    await expect($(".ide-workbench")).toBeExisting();
  });

  it("renders the primary Work-first navigation", async () => {
    const work = $("[aria-label^='Work —']");
    await expect(work).toBeExisting();
    await expect(work).toHaveAttribute("aria-pressed", "true");

    await expect($("[aria-label^='Code —']")).toBeExisting();
    await expect($("[aria-label^='Changes —']")).toBeExisting();
    await expect($("[aria-label^='Runs —']")).toBeExisting();
    await expect($("[aria-label^='Projects —']")).toBeExisting();
  });

  it("funnels a throwaway workspace into Work Scope through one project action", async () => {
    const welcomeDialog = $(".app-dialog[role='dialog']");
    if (await welcomeDialog.isExisting()) {
      await expect(welcomeDialog).toBeDisplayed();
      await expect(welcomeDialog.$("h2")).toHaveText("Your local-first engineering workspace");
      const close = welcomeDialog.$("button[aria-label='Close']");
      await close.waitForClickable({ timeout: 5_000 });
      await close.click();
      await welcomeDialog.waitForExist({ reverse: true, timeout: 5_000 });
    }

    const work = $("[aria-label^='Work —']");
    if ((await work.getAttribute("aria-pressed")) !== "true") {
      await work.click();
    }
    await expect(work).toHaveAttribute("aria-pressed", "true");

    const scopeHeading = $("//h2[normalize-space()='Scope']");
    await scopeHeading.waitForExist({ timeout: 30_000 });
    await expect(scopeHeading).toBeDisplayed();

    const phases = $("[aria-label='Task phases']");
    await expect(phases).toBeExisting();
    await expect(phases.$("[aria-label='Phase Scope: Current']")).toHaveText("Scope");

    const connectProject = $("//button[normalize-space()='Connect a project']");
    await expect(connectProject).toBeDisplayed();
    await expect($(".semantic-action-bar__primary .primary-cta")).not.toBeExisting();

    await connectProject.click();
    await expect($("[aria-label^='Projects —']")).toHaveAttribute("aria-pressed", "true");
  });
});
