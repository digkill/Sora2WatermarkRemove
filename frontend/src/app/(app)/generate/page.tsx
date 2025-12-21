"use client";

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { listUploads, uploadVideo } from "@/lib/api";
import { getToken } from "@/lib/auth";

export default function GeneratePage() {
  const router = useRouter();
  const [url, setUrl] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [uploads, setUploads] = useState<Awaited<ReturnType<typeof listUploads>>>([]);
  const [uploadsLoading, setUploadsLoading] = useState(false);
  const [uploadsOffset, setUploadsOffset] = useState(0);
  const [uploadsHasMore, setUploadsHasMore] = useState(true);

  useEffect(() => {
    if (!getToken()) {
      router.push("/login");
    }
  }, [router]);

  const refreshUploads = async () => {
    setUploadsLoading(true);
    try {
      const data = await listUploads(50, 0);
      setUploads(data);
      setUploadsOffset(data.length);
      setUploadsHasMore(data.length === 50);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load uploads");
    } finally {
      setUploadsLoading(false);
    }
  };

  const loadMoreUploads = async () => {
    if (!uploadsHasMore || uploadsLoading) {
      return;
    }
    setUploadsLoading(true);
    try {
      const data = await listUploads(50, uploadsOffset);
      setUploads((prev) => [...prev, ...data]);
      setUploadsOffset((prev) => prev + data.length);
      setUploadsHasMore(data.length === 50);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load uploads");
    } finally {
      setUploadsLoading(false);
    }
  };

  useEffect(() => {
    if (getToken()) {
      refreshUploads();
    }
  }, []);

  const handleUpload = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!url.trim()) {
      setError("Paste a video URL first.");
      return;
    }
    setError(null);
    setStatus(null);
    setLoading(true);
    try {
      const result = await uploadVideo({
        url: url.trim(),
      });
      setStatus(`Processing started. Task ID: ${result.task_id}`);
      refreshUploads();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Upload failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="grid gap-6 lg:grid-cols-[1.2fr_0.8fr]">
      <Card className="border-border/60 bg-white/80">
        <CardHeader>
          <CardTitle className="text-2xl font-[var(--font-display)]">Generate a clean file</CardTitle>
          <CardDescription>Paste a Sora share link and we handle the rest.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <form className="space-y-4" onSubmit={handleUpload}>
            <div className="space-y-2">
              <label className="text-sm text-muted-foreground" htmlFor="url">
                Sora share URL
              </label>
              <Input
                id="url"
                type="url"
                value={url}
                onChange={(event) => setUrl(event.target.value)}
                placeholder="https://sora.chatgpt.com/p/..."
              />
            </div>
            <Button type="submit" disabled={loading}>
              {loading ? "Uploading..." : "Start processing"}
            </Button>
          </form>
          {status && <p className="text-sm text-foreground">{status}</p>}
          {error && <p className="text-sm text-destructive">{error}</p>}
        </CardContent>
      </Card>

      <div className="space-y-6">
        <Card className="border-border/60 bg-white/80">
          <CardHeader>
            <CardTitle className="text-xl font-[var(--font-display)]">How credits work</CardTitle>
            <CardDescription>Automatic handling between monthly and one-time credits.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-muted-foreground">
            <div className="flex items-center justify-between rounded-2xl border border-border/60 bg-background/60 px-4 py-3">
              <span>Monthly quota</span>
              <Badge variant="secondary">Used first</Badge>
            </div>
            <div className="flex items-center justify-between rounded-2xl border border-border/60 bg-background/60 px-4 py-3">
              <span>One-time credits</span>
              <Badge variant="secondary">Backup</Badge>
            </div>
            <p>
              If you need more removals, return to the dashboard and add a pack or subscription.
            </p>
          </CardContent>
        </Card>

        <Card className="border-border/60 bg-white/80">
          <CardHeader className="flex flex-row items-center justify-between gap-4">
            <div>
              <CardTitle className="text-xl font-[var(--font-display)]">Recent uploads</CardTitle>
              <CardDescription>Track all generated videos and download results.</CardDescription>
            </div>
            <Button size="sm" variant="outline" onClick={refreshUploads} disabled={uploadsLoading}>
              {uploadsLoading ? "Refreshing..." : "Refresh"}
            </Button>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-muted-foreground">
            {uploads.length === 0 && <p>No uploads yet.</p>}
            {uploads.map((item) => (
              <div
                key={item.id}
                className="flex flex-col gap-2 rounded-2xl border border-border/60 bg-background/60 px-4 py-3 sm:flex-row sm:items-center sm:justify-between"
              >
                <div>
                  <p className="text-sm font-medium text-foreground">{item.original_filename}</p>
                  <p className="text-xs text-muted-foreground">Status: {item.status}</p>
                </div>
                {item.cleaned_url ? (
                  <Button asChild size="sm">
                    <a href={item.cleaned_url} target="_blank" rel="noreferrer">
                      Download
                    </a>
                  </Button>
                ) : (
                  <Badge variant="secondary">Processing</Badge>
                )}
              </div>
            ))}
            {uploadsHasMore && (
              <Button
                size="sm"
                variant="outline"
                className="w-full"
                onClick={loadMoreUploads}
                disabled={uploadsLoading}
              >
                {uploadsLoading ? "Loading..." : "Load more"}
              </Button>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
