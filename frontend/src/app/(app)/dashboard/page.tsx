"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { createPayment, getProducts, listSubscriptions, cancelSubscription, getCreditsStatus } from "@/lib/api";
import { getToken } from "@/lib/auth";

type ViewState = "idle" | "loading" | "error";

export default function DashboardPage() {
  const router = useRouter();
  const [productsState, setProductsState] = useState<ViewState>("idle");
  const [subsState, setSubsState] = useState<ViewState>("idle");
  const [products, setProducts] = useState<Awaited<ReturnType<typeof getProducts>>>([]);
  const [subscriptions, setSubscriptions] = useState<
    Awaited<ReturnType<typeof listSubscriptions>>
  >([]);
  const [credits, setCredits] = useState<Awaited<ReturnType<typeof getCreditsStatus>> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [paymentMessage, setPaymentMessage] = useState<string | null>(null);
  const disableSubscriptions = process.env.NEXT_PUBLIC_DISABLE_SUBSCRIPTIONS === "true";

  useEffect(() => {
    if (!getToken()) {
      router.push("/login");
    }
  }, [router]);

  useEffect(() => {
    const loadProducts = async () => {
      if (!getToken()) {
        return;
      }
      setProductsState("loading");
      try {
        const data = await getProducts();
        setProducts(data);
        setProductsState("idle");
      } catch (err) {
        setProductsState("error");
        setError(err instanceof Error ? err.message : "Failed to load products");
      }
    };
    loadProducts();
  }, []);

  useEffect(() => {
    const loadSubscriptions = async () => {
      if (!getToken()) {
        return;
      }
      if (disableSubscriptions) {
        setSubscriptions([]);
        setSubsState("idle");
        return;
      }
      setSubsState("loading");
      try {
        const data = await listSubscriptions();
        setSubscriptions(data);
        setSubsState("idle");
      } catch (err) {
        setSubsState("error");
        setError(err instanceof Error ? err.message : "Failed to load subscriptions");
      }
    };
    loadSubscriptions();
  }, [disableSubscriptions]);

  useEffect(() => {
    const loadCredits = async () => {
      if (!getToken()) {
        return;
      }
      try {
        const data = await getCreditsStatus();
        setCredits(data);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to load credits");
      }
    };
    loadCredits();
  }, []);

  const grouped = useMemo(() => {
    return {
      oneTime: products.filter((item) => item.product_type === "one_time"),
      subscription: products.filter((item) => item.product_type === "subscription"),
    };
  }, [products]);

  const featuredPrice = "14.99";
  const sortedOneTime = useMemo(() => {
    return [...grouped.oneTime].sort((a, b) => {
      const aIsTen = a.name.includes("10") || a.slug.includes("10");
      const bIsTen = b.name.includes("10") || b.slug.includes("10");
      if (aIsTen && !bIsTen) return -1;
      if (!aIsTen && bIsTen) return 1;
      const aCredits = a.credits_granted ?? Number.MAX_SAFE_INTEGER;
      const bCredits = b.credits_granted ?? Number.MAX_SAFE_INTEGER;
      return aCredits - bCredits;
    });
  }, [grouped.oneTime]);

  const handleBuy = async (slug: string) => {
    setPaymentMessage(null);
    setError(null);
    try {
      const result = await createPayment({ product_slug: slug });
      if (result.payment_url) {
        window.open(result.payment_url, "_blank", "noopener,noreferrer");
        setPaymentMessage("Payment link opened in a new tab.");
      } else {
        setPaymentMessage("Payment created. Check your Lava dashboard.");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Payment failed");
    }
  };

  const handleCancel = async (id: number) => {
    setError(null);
    try {
      await cancelSubscription(id);
      const updated = await listSubscriptions();
      setSubscriptions(updated);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Cancel failed");
    }
  };

  return (
    <div className="space-y-10">
      <section className="grid gap-6 lg:grid-cols-[1.3fr_0.7fr]">
        <Card className="border-border/60 bg-white/80">
          <CardHeader>
            <CardTitle className="text-2xl font-[var(--font-display)]">Welcome to your desk</CardTitle>
            <CardDescription>Manage your credits, subscriptions, and upcoming uploads.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4 text-sm text-muted-foreground">
            {credits && (
              <div className="grid gap-3 sm:grid-cols-3">
                <div className="rounded-2xl border border-border/60 bg-background/60 px-4 py-3">
                  <p className="text-xs uppercase text-muted-foreground">One-time credits</p>
                  <p className="text-lg font-semibold text-foreground">{credits.credits}</p>
                </div>
                <div className="rounded-2xl border border-border/60 bg-background/60 px-4 py-3">
                  <p className="text-xs uppercase text-muted-foreground">Monthly quota</p>
                  <p className="text-lg font-semibold text-foreground">{credits.monthly_quota}</p>
                </div>
                <div className="rounded-2xl border border-border/60 bg-background/60 px-4 py-3">
                  <p className="text-xs uppercase text-muted-foreground">Free generation</p>
                  <p className="text-lg font-semibold text-foreground">
                    {credits.free_generation_used ? "Used" : "Available"}
                  </p>
                </div>
              </div>
            )}
            <p>
              Use the <Link className="text-foreground underline" href="/generate">Generate</Link>{" "}
              page to upload a new Sora video. Credits apply automatically.
            </p>
            <div className="flex flex-wrap gap-3">
              <Badge variant="secondary">Credits stack with subscriptions</Badge>
              <Badge variant="secondary">Instant payment links</Badge>
              <Badge variant="secondary">Secure processing</Badge>
            </div>
          </CardContent>
        </Card>
        <Card className="border-border/60 bg-white/80">
          <CardHeader>
            <CardTitle className="text-xl font-[var(--font-display)]">Quick actions</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Button asChild className="w-full">
              <Link href="/generate">Upload a video</Link>
            </Button>
            <Button asChild variant="outline" className="w-full">
              <Link href="/generate">Check processing status</Link>
            </Button>
          </CardContent>
        </Card>
      </section>

      {error && <p className="text-sm text-destructive">{error}</p>}
      {paymentMessage && <p className="text-sm text-foreground">{paymentMessage}</p>}

      <section className="grid gap-6 ">
        <Card className="border-border/60 bg-white/80">
          <CardHeader>
            <CardTitle className="text-xl font-[var(--font-display)]">One-time packs</CardTitle>
            <CardDescription>Top up when you run out of monthly quota.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {productsState === "loading" && <p className="text-sm text-muted-foreground">Loading packs...</p>}
            {productsState === "idle" && (
              <div className="grid gap-4 md:grid-cols-3">
                {sortedOneTime.map((product) => {
                  const isFeatured = product.price === featuredPrice;
                  return (
                    <div
                      key={product.id}
                      className={`relative flex h-full flex-col justify-between rounded-2xl border border-border/60 bg-background/70 px-5 py-4 ${
                        isFeatured ? "ring-2 ring-foreground shadow-lg" : ""
                      }`}
                    >
                      {isFeatured && (
                        <Badge className="absolute right-4 top-4 bg-foreground text-background">
                          Top pick
                        </Badge>
                      )}
                      <div className="space-y-2">
                        <p className="text-lg font-semibold">{product.name}</p>
                        <p className="text-sm text-muted-foreground">
                          {product.description ?? "One-time credit pack"}
                        </p>
                      </div>
                      <div className="mt-4 flex items-center justify-between">
                        <p className="text-base font-semibold">
                          {product.price} {product.currency}
                        </p>
                        <Button size="sm" onClick={() => handleBuy(product.slug)}>
                          Buy
                        </Button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
            {productsState === "idle" && grouped.oneTime.length === 0 && (
              <p className="text-sm text-muted-foreground">No packs available.</p>
            )}
          </CardContent>
        </Card>

        {!disableSubscriptions && (
          <Card className="border-border/60 bg-white/80">
            <CardHeader>
              <CardTitle className="text-xl font-[var(--font-display)]">Subscriptions</CardTitle>
              <CardDescription>Monthly quota that refreshes automatically.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {productsState === "loading" && <p className="text-sm text-muted-foreground">Loading plans...</p>}
              {productsState === "idle" &&
                grouped.subscription.map((product) => (
                  <div key={product.id} className="flex items-center justify-between rounded-2xl border border-border/60 bg-background/60 px-4 py-3">
                    <div>
                      <p className="font-medium">{product.name}</p>
                      <p className="text-xs text-muted-foreground">{product.description ?? "Monthly subscription"}</p>
                    </div>
                    <Button size="sm" onClick={() => handleBuy(product.slug)}>
                      Subscribe {product.price} {product.currency}
                    </Button>
                  </div>
                ))}
              {productsState === "idle" && grouped.subscription.length === 0 && (
                <p className="text-sm text-muted-foreground">No subscriptions available.</p>
              )}
            </CardContent>
          </Card>
        )}
      </section>

      {!disableSubscriptions && (
        <section className="grid gap-6">
          <Card className="border-border/60 bg-white/80">
            <CardHeader>
              <CardTitle className="text-xl font-[var(--font-display)]">Active subscriptions</CardTitle>
              <CardDescription>Monitor status and cancel when needed.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {subsState === "loading" && <p className="text-sm text-muted-foreground">Loading subscriptions...</p>}
              {subsState === "idle" && subscriptions.length === 0 && (
                <p className="text-sm text-muted-foreground">No subscriptions on this account.</p>
              )}
              {subsState === "idle" &&
                subscriptions.map((sub) => (
                  <div key={sub.id} className="flex flex-col gap-3 rounded-2xl border border-border/60 bg-background/60 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
                    <div>
                      <p className="font-medium">Subscription #{sub.id}</p>
                      <p className="text-xs text-muted-foreground">Status: {sub.status}</p>
                      <p className="text-xs text-muted-foreground">
                        Period end: {sub.current_period_end ? new Date(sub.current_period_end).toLocaleDateString() : "N/A"}
                      </p>
                    </div>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => handleCancel(sub.id)}
                      disabled={sub.status === "canceled"}
                    >
                      {sub.status === "canceled" ? "Canceled" : "Cancel"}
                    </Button>
                  </div>
                ))}
            </CardContent>
          </Card>
        </section>
      )}
    </div>
  );
}
