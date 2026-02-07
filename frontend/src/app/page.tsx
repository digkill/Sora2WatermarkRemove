import Link from "next/link";
import { ArrowRight, Check, Sparkles } from "lucide-react";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

const features = [
  {
    title: "Fast turnaround",
    description: "Upload, process, and get a clean file in minutes. No manual steps or complex exports.",
  },
  {
    title: "Smart credit logic",
    description: "Subscriptions use monthly quota first. One-time packs top up when you run out.",
  },
  {
    title: "Creator-friendly output",
    description: "High-quality exports optimized for short-form and long-form workflows.",
  },
];

const pricing = [
  {
    name: "10 Removals Pack",
    price: "5.00 USD",
    tag: "One-time",
    description: "Remove watermark from 10 videos",
    credits: "10 removals",
    highlights: ["Instant delivery", "No subscription", "Credits never expire"],
  },
  {
    name: "25 Removals Pack",
    price: "14.99 USD",
    tag: "Top pick",
    description: "Best value for multiple videos, remove watermark from 25 videos",
    credits: "25 removals",
    highlights: ["Best value", "One-time purchase", "Priority queue"],
    featured: true,
  },
  {
    name: "50 Removals Pack",
    price: "29.99 USD",
    tag: "One-time",
    description: "For heavy users, remove watermark from 50 videos",
    credits: "50 removals",
    highlights: ["High volume", "Fast processing", "Priority support"],
  },
];

const steps = [
  {
    title: "Upload your Sora video",
    copy: "Drop the file or paste a link. We keep your originals secure.",
  },
  {
    title: "Choose a plan",
    copy: "Use a monthly subscription or grab a one-time pack.",
  },
  {
    title: "Download the clean cut",
    copy: "We remove the watermark and return a crisp, share-ready file.",
  },
];

export default function Home() {
  const disableSubscriptions = process.env.NEXT_PUBLIC_DISABLE_SUBSCRIPTIONS === "true";
  const visiblePricing = pricing;

  return (
    <div className="relative min-h-screen overflow-hidden">
      <div className="absolute inset-0 -z-10 bg-[radial-gradient(circle_at_top,_rgba(240,224,180,0.35),_transparent_55%),radial-gradient(circle_at_10%_80%,_rgba(168,197,255,0.35),_transparent_40%),radial-gradient(circle_at_80%_70%,_rgba(255,189,149,0.35),_transparent_45%)]" />

      <header className="mx-auto flex w-full max-w-6xl items-center justify-between px-6 py-6">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-foreground text-background">
            <Sparkles className="h-5 w-5" />
          </div>
          <div>
            <p className="text-sm uppercase tracking-[0.2em] text-muted-foreground">Sora Clean</p>
            <p className="text-lg font-semibold">Watermark Studio</p>
          </div>
        </div>
        <div className="hidden items-center gap-3 md:flex">
          <Button asChild variant="ghost">
            <Link href="/login">Sign In</Link>
          </Button>
          <Button asChild className="gap-2">
            <Link href="/register">
              Start Clean
              <ArrowRight className="h-4 w-4" />
            </Link>
          </Button>
        </div>
      </header>

      <main className="mx-auto flex w-full max-w-6xl flex-col gap-20 px-6 pb-20">
        <section className="grid gap-12 pt-10 lg:grid-cols-[1.1fr_0.9fr] lg:items-center">
          <div className="flex flex-col gap-6">
            <Badge className="w-fit bg-foreground text-background">Built for Sora creators</Badge>
            <h1 className="text-4xl font-[var(--font-display)] leading-tight sm:text-5xl lg:text-6xl">
              Remove Sora watermarks with a clean, modern workflow.
            </h1>
            <p className="max-w-xl text-lg text-muted-foreground">
              A streamlined backend and a friendly interface for creators who want fast, reliable
              watermark removal. Pay per pack or subscribe monthly - your credits always stay in sync.
            </p>
            <div className="flex flex-wrap gap-4">
              <Button asChild size="lg" className="gap-2">
                <Link href="/register">
                  Get started
                  <ArrowRight className="h-4 w-4" />
                </Link>
              </Button>
              <Button asChild size="lg" variant="outline">
                <Link href="#pricing">View pricing</Link>
              </Button>
            </div>
            <div className="flex flex-wrap gap-6 text-sm text-muted-foreground">
              <span>Average delivery: 4-7 minutes</span>
              <span>Files are auto-deleted after processing</span>
              <span>Credits stack with subscriptions</span>
            </div>
          </div>
          <div className="relative">
            <div className="absolute -left-10 top-6 h-24 w-24 rounded-full bg-accent blur-2xl" />
            <Card className="border-white/40 bg-white/70 backdrop-blur">
              <CardHeader>
                <CardTitle className="text-2xl font-[var(--font-display)]">Processing queue</CardTitle>
                <CardDescription>Live status for recent uploads.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                {[
                  { name: "Cyber Coastline", status: "Cleaning", progress: "68%" },
                  { name: "Neon Corridor", status: "Queued", progress: "12%" },
                  { name: "Midnight Run", status: "Complete", progress: "100%" },
                ].map((item) => (
                  <div key={item.name} className="flex items-center justify-between rounded-2xl border border-border/60 bg-background/60 px-4 py-3">
                    <div>
                      <p className="text-sm font-medium">{item.name}</p>
                      <p className="text-xs text-muted-foreground">{item.status}</p>
                    </div>
                    <Badge variant="secondary">{item.progress}</Badge>
                  </div>
                ))}
              </CardContent>
            </Card>
          </div>
        </section>

        <section className="grid gap-6 lg:grid-cols-3">
          {features.map((feature) => (
            <Card key={feature.title} className="border-white/60 bg-white/70">
              <CardHeader>
                <CardTitle className="text-xl font-[var(--font-display)]">{feature.title}</CardTitle>
                <CardDescription>{feature.description}</CardDescription>
              </CardHeader>
              <CardContent>
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                  <Check className="h-4 w-4 text-foreground" />
                  Secure processing with automatic cleanup
                </div>
              </CardContent>
            </Card>
          ))}
        </section>

        <section className="rounded-[32px] border border-border/60 bg-white/70 px-6 py-12 backdrop-blur">
          <div className="flex flex-col gap-4 text-center">
            <p className="text-sm uppercase tracking-[0.3em] text-muted-foreground">How it works</p>
            <h2 className="text-3xl font-[var(--font-display)]">Three steps to a clean file</h2>
          </div>
          <div className="mt-10 grid gap-6 lg:grid-cols-3">
            {steps.map((step, index) => (
              <div key={step.title} className="space-y-3 rounded-2xl border border-border/60 bg-background/70 p-6">
                <Badge variant="secondary">Step {index + 1}</Badge>
                <h3 className="text-xl font-[var(--font-display)]">{step.title}</h3>
                <p className="text-sm text-muted-foreground">{step.copy}</p>
              </div>
            ))}
          </div>
        </section>

        <section id="pricing" className="space-y-8">
          <div className="flex flex-col items-center gap-3 text-center">
            <p className="text-sm uppercase tracking-[0.3em] text-muted-foreground">One-time packs</p>
            <h2 className="text-3xl font-[var(--font-display)]">
              Top up when you run out of monthly quota.
            </h2>
            <p className="max-w-xl text-sm text-muted-foreground">
              {disableSubscriptions
                ? "Grab a one-time pack and keep your workflow moving."
                : "Add a pack anytime — one-time credits never expire."}
            </p>
          </div>
          <div className="grid gap-6 lg:grid-cols-3">
            {visiblePricing.map((plan) => (
              <Card
                key={plan.name}
                className={`relative overflow-hidden border-border/70 bg-white/80 ${
                  plan.featured ? "ring-2 ring-foreground shadow-xl" : ""
                }`}
              >
                {plan.featured && (
                  <Badge className="absolute right-4 top-4 bg-foreground text-background">
                    Top pick
                  </Badge>
                )}
                <CardHeader>
                  <CardTitle className="text-2xl font-[var(--font-display)]">{plan.name}</CardTitle>
                  <CardDescription>{plan.description}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-6">
                  <div>
                    <p className="text-3xl font-semibold">{plan.price}</p>
                    <p className="text-sm text-muted-foreground">{plan.credits}</p>
                    <Badge variant="secondary" className="mt-2">
                      {plan.tag}
                    </Badge>
                  </div>
                  <div className="space-y-2">
                    {plan.highlights.map((item) => (
                      <div key={item} className="flex items-center gap-2 text-sm">
                        <Check className="h-4 w-4 text-foreground" />
                        {item}
                      </div>
                    ))}
                  </div>
                  <Button asChild className="w-full">
                    <Link href="/register">Buy</Link>
                  </Button>
                </CardContent>
              </Card>
            ))}
          </div>
        </section>

        <section className="grid gap-8 rounded-[32px] border border-border/60 bg-white/70 px-6 py-10 lg:grid-cols-[1.1fr_0.9fr]">
          <div className="space-y-3">
            <p className="text-sm uppercase tracking-[0.3em] text-muted-foreground">FAQ</p>
            <h2 className="text-3xl font-[var(--font-display)]">Questions, answered</h2>
            <p className="text-sm text-muted-foreground">
              We keep payments and credits transparent. If you want a custom workflow, reach out.
            </p>
          </div>
          <Accordion type="single" collapsible className="w-full">
            <AccordionItem value="item-1">
              <AccordionTrigger>Can I combine a subscription with packs?</AccordionTrigger>
              <AccordionContent>
                Yes. Monthly quota is used first, and one-time credits kick in automatically when
                your subscription runs out.
              </AccordionContent>
            </AccordionItem>
            <AccordionItem value="item-2">
              <AccordionTrigger>How fast is delivery?</AccordionTrigger>
              <AccordionContent>
                Most videos are processed in 4-7 minutes. Large files may take longer, but you can
                monitor progress in your dashboard.
              </AccordionContent>
            </AccordionItem>
            <AccordionItem value="item-3">
              <AccordionTrigger>Is my content secure?</AccordionTrigger>
              <AccordionContent>
                Files are encrypted in transit, stored only during processing, and auto-deleted
                after completion.
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </section>

        <section className="grid gap-6 rounded-[32px] border border-border/60 bg-foreground px-6 py-10 text-background lg:grid-cols-[1.3fr_0.7fr] lg:items-center">
          <div className="space-y-3">
            <Badge className="bg-background text-foreground">Ready to start?</Badge>
            <h2 className="text-3xl font-[var(--font-display)]">
              Get watermark-free in your next upload.
            </h2>
            <p className="text-sm text-background/80">
              Join creators who ship faster with Sora Clean. Add credits or subscribe in minutes.
            </p>
          </div>
          <div className="flex flex-col gap-3">
            <Input placeholder="Email for early access" className="bg-background text-foreground" />
            <Button className="gap-2">
              Join waitlist
              <ArrowRight className="h-4 w-4" />
            </Button>
          </div>
        </section>
      </main>

      <footer className="mx-auto w-full max-w-6xl border-t border-border/50 px-6 py-8 text-sm text-muted-foreground">
        <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
          <p>(c) 2025 Sora Clean. All rights reserved.</p>
          <div className="flex gap-6">
            <span>Terms</span>
            <span>Privacy</span>
            <span>Support</span>
          </div>
        </div>
      </footer>
    </div>
  );
}
