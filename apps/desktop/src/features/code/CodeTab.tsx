import { useState } from "react";
import { formatNumber, FileGroup, stringifyPreview } from "../../shared/ui/SharedComponents";
import { useCode } from "./useCode";
import { callCommand } from "../../shared/api/queries";

export function CodeTab() {
  const { codeWorkbench, changedFiles, isLoading: isBusy } = useCode();
  const [selectedFile, setSelectedFile] = useState("");
  const [selectedFileContent, setSelectedFileContent] = useState("");

  const loadCodeFile = async (path: string) => {
    setSelectedFile(path);
    setSelectedFileContent("Loading...");
    try {
      const result = await callCommand<any>("read_code_file", { relativePath: path, relative_path: path });
      setSelectedFileContent(result?.content ?? stringifyPreview(result));
    } catch (error) {
      setSelectedFileContent(String(error));
    }
  };

  const refreshAll = () => { };

  return (
    <div className="content-grid two-column-grid">
      <div className="left-column">
        <section className="hero-panel">
          <p className="eyebrow">Code workbench</p>
          <h1>{formatNumber(changedFiles.length)} files changed.</h1>
          <p className="lead">Review unstaged and staged edits before building context. Commits are not handled here.</p>
          <div className="button-row">
            <button className="ghost-button" onClick={() => void refreshAll()} disabled={isBusy}>Refresh code</button>
          </div>
        </section>
        <section className="panel">
          <FileGroup title="Changed files" files={changedFiles} />
        </section>
      </div>
      <div className="right-column">
        <section className="panel fill-height preview-panel">
          <div className="panel-title-row">
            <p className="eyebrow">File preview</p>
            {selectedFile && <strong>{selectedFile}</strong>}
          </div>
          <pre className="code-panel scrollable">{selectedFileContent || "Select a file to preview"}</pre>
        </section>
      </div>
    </div>
  );
}
