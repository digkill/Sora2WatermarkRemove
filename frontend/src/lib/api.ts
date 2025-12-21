import { getToken } from "./auth";

const API_BASE = process.env.NEXT_PUBLIC_API_BASE_URL;

type RequestOptions = RequestInit & {
  auth?: boolean;
};

export type Product = {
  id: number;
  slug: string;
  name: string;
  description: string | null;
  price: string;
  currency: string;
  product_type: "one_time" | "subscription";
  credits_granted: number | null;
  monthly_credits: number | null;
};

export type Subscription = {
  id: number;
  user_id: number;
  product_id: number;
  provider: string;
  provider_subscription_id: string | null;
  status: string;
  current_period_start: string | null;
  current_period_end: string | null;
  canceled_at: string | null;
};

export type AuthResponse = {
  token?: string | null;
  user_id: number;
  verification_required: boolean;
};

export type UploadResponse = {
  message: string;
  upload_id: number;
  task_id: string;
};

export type CreditsStatus = {
  credits: number;
  monthly_quota: number;
  free_generation_used: boolean;
};

export type UploadItem = {
  id: number;
  status: string;
  original_filename: string;
  cleaned_url?: string | null;
  created_at?: string | null;
};

async function apiFetch<T>(path: string, options: RequestOptions = {}): Promise<T> {
  if (!API_BASE) {
    throw new Error("NEXT_PUBLIC_API_BASE_URL is not set");
  }
  const headers = new Headers(options.headers);
  if (options.auth) {
    const token = getToken();
    if (token) {
      headers.set("Authorization", `Bearer ${token}`);
    }
  }
  if (options.body && !headers.has("Content-Type") && !(options.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }

  const res = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers,
  });

  if (!res.ok) {
    const text = await res.text();
    let message = text || `Request failed (${res.status})`;
    try {
      const parsed = JSON.parse(text);
      if (parsed?.error) {
        message = parsed.error;
      }
    } catch {
      // ignore
    }
    throw new Error(message);
  }

  if (res.status === 204) {
    return {} as T;
  }

  return res.json() as Promise<T>;
}

export async function register(payload: {
  email: string;
  password: string;
  username?: string;
}): Promise<AuthResponse> {
  return apiFetch<AuthResponse>("/auth/register", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function login(payload: { email: string; password: string }): Promise<AuthResponse> {
  return apiFetch<AuthResponse>("/auth/login", {
    method: "POST",
    body: JSON.stringify(payload),
  });
}

export async function verifyEmail(token: string) {
  return apiFetch("/auth/verify?token=" + encodeURIComponent(token));
}

export async function resendVerification(email: string) {
  return apiFetch("/auth/resend-verification", {
    method: "POST",
    body: JSON.stringify({ email }),
  });
}

export async function getProducts(): Promise<Product[]> {
  return apiFetch<Product[]>("/api/products", { auth: true });
}

export async function createPayment(payload: {
  product_slug: string;
  buyer_email?: string;
  periodicity?: string;
}): Promise<{ payment_url?: string; transaction_id: number }> {
  return apiFetch("/api/create-payment", {
    method: "POST",
    auth: true,
    body: JSON.stringify(payload),
  });
}

export async function listSubscriptions(): Promise<Subscription[]> {
  return apiFetch<Subscription[]>("/api/subscriptions", { auth: true });
}

export async function cancelSubscription(subscription_id: number) {
  return apiFetch("/api/subscriptions/cancel", {
    method: "POST",
    auth: true,
    body: JSON.stringify({ subscription_id }),
  });
}

export async function uploadVideo(payload: { url: string }): Promise<UploadResponse> {
  const form = new FormData();
  form.append("url", payload.url);
  return apiFetch<UploadResponse>("/api/upload", {
    method: "POST",
    auth: true,
    body: form,
  });
}

export async function getCreditsStatus(): Promise<CreditsStatus> {
  return apiFetch<CreditsStatus>("/api/credits", { auth: true });
}

export async function listUploads(limit = 100, offset = 0): Promise<UploadItem[]> {
  return apiFetch<UploadItem[]>(`/api/uploads?limit=${limit}&offset=${offset}`, { auth: true });
}
