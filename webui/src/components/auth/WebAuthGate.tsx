import { Cloud, Lock } from "lucide-react";
import { FormEvent, ReactNode, useEffect, useRef, useState } from "react";
import { useLoginMutation, usePublicSettingsQuery } from "../../lib/query/auth";
import { codexLocalCopy } from "../../lib/domain/codexViewModel";
import type { SessionUser } from "../../types";

type TurnstileWidgetId = string;

declare global {
  interface Window {
    turnstile?: {
      render: (container: HTMLElement, options: {
        sitekey: string;
        action?: string;
        theme?: "dark" | "light" | "auto";
        callback?: (token: string) => void;
        "expired-callback"?: () => void;
        "error-callback"?: () => void;
      }) => TurnstileWidgetId;
      reset: (widgetId?: TurnstileWidgetId) => void;
      remove?: (widgetId: TurnstileWidgetId) => void;
    };
  }
}

export function WebAuthGate({
  session,
  webAuth,
  onLogin,
  children
}: {
  session: SessionUser | null;
  webAuth: boolean;
  onLogin: (user: SessionUser) => void;
  children: ReactNode;
}) {
  if (!session && webAuth) return <LoginScreen onLogin={onLogin} />;
  if (!session) return null;
  return <>{children}</>;
}

function LoginScreen({ onLogin }: { onLogin: (user: SessionUser) => void }) {
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [turnstileToken, setTurnstileToken] = useState("");
  const [turnstileStatus, setTurnstileStatus] = useState<"idle" | "loading" | "ready" | "verified" | "error">("idle");
  const widgetRef = useRef<TurnstileWidgetId | null>(null);
  const turnstileRef = useRef<HTMLDivElement | null>(null);
  const publicSettings = usePublicSettingsQuery();
  const turnstileEnabled = Boolean(publicSettings.data?.turnstile_enabled && publicSettings.data.turnstile_site_key);
  const turnstileRequired = Boolean(publicSettings.data?.turnstile_required);
  const turnstileAction = publicSettings.data?.turnstile_action || "login";

  useEffect(() => {
    if (!turnstileEnabled || !publicSettings.data?.turnstile_site_key || !turnstileRef.current) {
      setTurnstileStatus("idle");
      setTurnstileToken("");
      return;
    }

    let cancelled = false;
    setTurnstileToken("");
    setTurnstileStatus("loading");
    ensureTurnstileScript()
      .then(() => {
        if (cancelled || !turnstileRef.current || !window.turnstile) return;
        if (widgetRef.current && window.turnstile.remove) {
          window.turnstile.remove(widgetRef.current);
          widgetRef.current = null;
        }
        turnstileRef.current.innerHTML = "";
        widgetRef.current = window.turnstile.render(turnstileRef.current, {
          sitekey: publicSettings.data.turnstile_site_key,
          action: turnstileAction,
          theme: "dark",
          callback: (token) => {
            setTurnstileToken(token);
            setTurnstileStatus("verified");
          },
          "expired-callback": () => {
            setTurnstileToken("");
            setTurnstileStatus("ready");
          },
          "error-callback": () => {
            setTurnstileToken("");
            setTurnstileStatus("error");
          }
        });
        setTurnstileStatus("ready");
      })
      .catch(() => {
        if (!cancelled) setTurnstileStatus("error");
      });

    return () => {
      cancelled = true;
      if (widgetRef.current && window.turnstile?.remove) {
        window.turnstile.remove(widgetRef.current);
        widgetRef.current = null;
      }
    };
  }, [turnstileAction, turnstileEnabled, publicSettings.data?.turnstile_site_key]);

  const resetTurnstile = () => {
    if (widgetRef.current && window.turnstile) {
      window.turnstile.reset(widgetRef.current);
      setTurnstileToken("");
      setTurnstileStatus("ready");
    }
  };

  const mutation = useLoginMutation(onLogin);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    if (turnstileEnabled && !turnstileToken.trim()) {
      setError("请先完成 Turnstile 验证");
      return;
    }
    mutation.mutate(
      { username, password, turnstileToken },
      {
        onError: (err) => {
          setError(err.message);
          resetTurnstile();
        }
      }
    );
  };

  return (
    <div className="login-shell">
      <form className="login-panel" onSubmit={submit}>
        <div className="brand-mark"><Cloud size={24} /></div>
        <h1>NexusHub</h1>
        <p>{codexLocalCopy.loginSubtitle}</p>
        <label>
          <span>管理员</span>
          <input value={username} onChange={(event) => setUsername(event.target.value)} autoComplete="username" />
        </label>
        <label>
          <span>密码</span>
          <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="current-password" />
        </label>
        {turnstileEnabled && (
          <div className="turnstile-box">
            <div ref={turnstileRef} />
            <span>
              {turnstileRequired ? "Turnstile 强制验证" : "Turnstile 登录验证"}
              {turnstileStatus === "verified" ? "：已完成" : turnstileStatus === "loading" ? "：加载中" : turnstileStatus === "error" ? "：加载失败" : ""}
            </span>
          </div>
        )}
        {error && <div className="inline-error">{error}</div>}
        <button className="primary-button" disabled={mutation.isPending}>
          <Lock size={18} />
          登录
        </button>
      </form>
    </div>
  );
}

function ensureTurnstileScript(): Promise<void> {
  if (window.turnstile) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const existing = document.getElementById("cloudflare-turnstile-script") as HTMLScriptElement | null;
    if (existing) {
      existing.addEventListener("load", () => resolve(), { once: true });
      existing.addEventListener("error", () => reject(new Error("Turnstile script failed")), { once: true });
      return;
    }
    const script = document.createElement("script");
    script.id = "cloudflare-turnstile-script";
    script.src = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";
    script.async = true;
    script.defer = true;
    script.onload = () => resolve();
    script.onerror = () => reject(new Error("Turnstile script failed"));
    document.head.appendChild(script);
  });
}

