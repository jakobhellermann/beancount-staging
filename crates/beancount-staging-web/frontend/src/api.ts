interface Transaction {
  index: number;
  content: string;
}

interface TransactionResponse {
  transaction: Transaction;
  expense_account: string | null;
}

interface InitResponse {
  items: Transaction[];
  current_index: number;
}

interface CommitResponse {
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

  async getTransaction(index: number): Promise<TransactionResponse> {
    const resp = await fetch(`/api/transaction/${index}`);
    if (!resp.ok) {
      throw new Error(`Failed to load transaction: ${resp.statusText}`);
    }
    return await resp.json();
  }

  async saveAccount(index: number, expenseAccount: string): Promise<void> {
    const resp = await fetch(`/api/transaction/${index}/account`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ expense_account: expenseAccount }),
    });

    if (!resp.ok) {
      throw new Error(`Failed to save account: ${resp.statusText}`);
    }
  }

  async commitTransaction(index: number): Promise<CommitResponse> {
    const resp = await fetch(`/api/transaction/${index}/commit`, {
      method: "POST",
    });

    if (!resp.ok) {
      const errorData = await resp.json().catch(() => null);
      const errorMsg = errorData?.error ?? resp.statusText;
      throw new Error(errorMsg);
    }

    return await resp.json();
  }
}

export type { Transaction, TransactionResponse, InitResponse, CommitResponse };
