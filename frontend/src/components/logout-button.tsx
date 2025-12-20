"use client";

import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/button";
import { clearToken } from "@/lib/auth";

export function LogoutButton() {
  const router = useRouter();

  return (
    <Button
      variant="ghost"
      onClick={() => {
        clearToken();
        router.push("/login");
      }}
    >
      Log out
    </Button>
  );
}
