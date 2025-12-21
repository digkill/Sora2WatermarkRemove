"use client";

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import Link from "next/link";
import { resendVerification, verifyEmail } from "@/lib/api";

function VerifyContent() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const token = searchParams.get("token");
  const initialEmail = searchParams.get("email") || "";
  const [email, setEmail] = useState(initialEmail);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    const runVerify = async () => {
      if (!token) {
        return;
      }
      setLoading(true);
      setError(null);
      try {
        await verifyEmail(token);
        setStatus("Email verified. You can now sign in.");
        setTimeout(() => {
          router.push("/login");
        }, 1200);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Verification failed");
      } finally {
        setLoading(false);
      }
    };
    runVerify();
  }, [token]);

  const handleResend = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    setStatus(null);
    setLoading(true);
    try {
      await resendVerification(email);
      setStatus("Verification email sent. Check your inbox.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Request failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card className="border-border/60 bg-white/80">
      <CardHeader>
        <CardTitle className="text-2xl font-[var(--font-display)]">Verify your email</CardTitle>
        <CardDescription>
          Confirm your email address to unlock your dashboard.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {token ? (
          <div className="space-y-2 text-sm">
            {loading && <p className="text-muted-foreground">Verifying...</p>}
            {status && (
              <p className="text-foreground">
                {status}{" "}
                <Link className="underline underline-offset-4" href="/login">
                  Sign in
                </Link>
                .
              </p>
            )}
            {error && <p className="text-destructive">{error}</p>}
          </div>
        ) : (
          <form className="space-y-3" onSubmit={handleResend}>
            <div className="space-y-2">
              <label className="text-sm text-muted-foreground" htmlFor="email">
                Email
              </label>
              <Input
                id="email"
                type="email"
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                required
              />
            </div>
            {status && <p className="text-sm text-foreground">{status}</p>}
            {error && <p className="text-sm text-destructive">{error}</p>}
            <Button type="submit" disabled={loading}>
              {loading ? "Sending..." : "Resend verification email"}
            </Button>
          </form>
        )}
      </CardContent>
    </Card>
  );
}

export default function VerifyPage() {
  return (
    <Suspense fallback={<div className="text-sm text-muted-foreground">Loading...</div>}>
      <VerifyContent />
    </Suspense>
  );
}
