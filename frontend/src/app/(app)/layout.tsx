import Link from "next/link";
import { Button } from "@/components/ui/button";
import { LogoutButton } from "@/components/logout-button";

export default function AppLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen">
      <header className="mx-auto flex w-full max-w-6xl items-center justify-between px-6 py-6">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-foreground text-background">
            SC
          </div>
          <div>
            <p className="text-sm uppercase tracking-[0.2em] text-muted-foreground">Sora Clean</p>
            <p className="text-lg font-semibold">Creator Desk</p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <Button asChild variant="ghost">
            <Link href="/dashboard">Dashboard</Link>
          </Button>
          <Button asChild variant="ghost">
            <Link href="/generate">Generate</Link>
          </Button>
          <LogoutButton />
        </div>
      </header>
      <main className="mx-auto w-full max-w-6xl px-6 pb-16">{children}</main>
    </div>
  );
}
