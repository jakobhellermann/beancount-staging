import type { Directive } from "./model/beancount";

export interface InitResponse {
  items: Directive[];
  current_index: number;
  available_accounts: string[];
}

export interface TransactionResponse {
  transaction: Directive;
  predicted_account?: string;
}

export interface TransactionPatch {
  account?: string;
  payee?: string;
  narration?: string;
}

export interface CommitResponse {
  ok: boolean;
  remaining_count: number;
}

export class ApiClient {
  async init(): Promise<InitResponse> {
    const resp = await fetch("/api/init");
    if (!resp.ok) {
      throw new Error(`Failed to initialize: ${resp.statusText}`);
    }
    return await resp.json();
  }

  async getTransaction(id: string): Promise<TransactionResponse> {
    const resp = await fetch(`/api/transaction/${id}`);
    if (!resp.ok) {
      throw new Error(`Failed to load transaction: ${resp.statusText}`);
    }
    return await resp.json();
  }

  async commitTransaction(id: string, patch: TransactionPatch): Promise<CommitResponse> {
    const resp = await fetch(`/api/transaction/${id}/commit`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(patch),
    });

    if (!resp.ok) {
      const errorData = await resp.json().catch(() => null);
      const errorMsg = errorData?.error ?? resp.statusText;
      throw new Error(errorMsg);
    }

    return await resp.json();
  }
}
