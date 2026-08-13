import { useEffect, useState } from "react";
import { callCommand } from "../../shared/api/queries";
import { formatNumber } from "../../shared/utils/helpers";
import { Dialog } from "./Dialog";

interface ArtifactContent {
  kind: string;
  title: string;
  path: string;
  exists: boolean;
  content: string;
  size_bytes: number;
}

export function ArtifactViewerModal({ kind, isOpen, onClose }: { kind: string; isOpen: boolean; onClose: () => void }) {
  const [data, setData] = useState<ArtifactContent | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) {
      setData(null);
      setLoading(false);
      setError(null);
      return;
    }
    let mounted = true;
    setData(null);
    setLoading(true);
    setError(null);
    callCommand<ArtifactContent>("read_artifact", { kind })
      .then((result) => { if (mounted) setData(result); })
      .catch((cause) => { if (mounted) setError(cause?.message || String(cause)); })
      .finally(() => { if (mounted) setLoading(false); });
    return () => { mounted = false; };
  }, [isOpen, kind]);

  return (
    <Dialog
      open={isOpen}
      title={data?.title || "Artifact"}
      eyebrow="Artifact viewer"
      onClose={onClose}
      maxWidth="900px"
    >
      {data ? <p className="muted app-dialog-artifact-meta">{data.path} · {formatNumber(data.size_bytes)} bytes</p> : null}
      {loading ? <p className="muted">Loading artifact content…</p> : null}
      {error ? <p className="notice danger" role="alert">Failed to load artifact: {error}</p> : null}
      {!loading && !error && data ? (
        data.exists ? <pre className="app-dialog-artifact">{data.content || "Empty file."}</pre> :
          <p className="muted">This artifact does not exist yet. Generate it from its owning workflow first.</p>
      ) : null}
    </Dialog>
  );
}
