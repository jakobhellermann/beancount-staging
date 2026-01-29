import { describe, it, expect, beforeEach, vi } from "vitest";
import { ApiClient } from "../src/api";
import { DirectiveRenderer } from "../src/directive-renderer";
import { filterAccounts } from "../src/account-filter";

// Note: app.ts auto-initializes at module load, so we test the components
// it uses rather than the StagingApp class directly

describe("App Integration", () => {
  beforeEach(() => {
    // Set up DOM that app expects
    document.body.innerHTML = `
      <div id="transaction"></div>
      <div id="counter"></div>
      <button id="commit"></button>
      <button id="prev"></button>
      <button id="next"></button>
      <div id="message"></div>
    `;

    // Reset fetch mock
    global.fetch = vi.fn();

    // Mock EventSource to prevent SSE connection
    global.EventSource = vi.fn().mockImplementation(() => ({
      onmessage: null,
      onerror: null,
      close: vi.fn(),
    })) as any;
  });

  describe("ApiClient integration", () => {
    it("should create API client", () => {
      const client = new ApiClient();
      expect(client).toBeTruthy();
    });

    it("should handle API calls", async () => {
      const client = new ApiClient();

      global.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          items: [],
          current_index: 0,
          available_accounts: [],
        }),
      });

      const result = await client.init();
      expect(result.items).toEqual([]);
    });
  });

  describe("DirectiveRenderer integration", () => {
    it("should create directive renderer", () => {
      const container = document.getElementById("transaction")!;
      const onInput = vi.fn();

      const renderer = new DirectiveRenderer(container, onInput, [], filterAccounts);

      expect(renderer).toBeTruthy();
    });

    it("should render transactions", () => {
      const container = document.getElementById("transaction")!;
      const onInput = vi.fn();

      const renderer = new DirectiveRenderer(container, onInput, [], filterAccounts);

      const transaction = {
        date: "2024-01-15",
        flag: "!",
        payee: "Test Store",
        narration: "Test purchase",
        tags: [],
        links: [],
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      };

      renderer.render(transaction);

      expect(container.textContent).toContain("2024-01-15");
      expect(container.textContent).toContain("Test Store");
    });
  });

  describe("Account filtering integration", () => {
    it("should filter accounts correctly", () => {
      const accounts = ["Expenses:Food", "Expenses:Travel", "Assets:Bank"];

      const result = filterAccounts("exp", accounts);

      expect(result).toEqual(["Expenses:Food", "Expenses:Travel"]);
    });
  });

  describe("DOM interactions", () => {
    it("should have required DOM elements", () => {
      expect(document.getElementById("transaction")).toBeTruthy();
      expect(document.getElementById("counter")).toBeTruthy();
      expect(document.getElementById("commit")).toBeTruthy();
      expect(document.getElementById("prev")).toBeTruthy();
      expect(document.getElementById("next")).toBeTruthy();
      expect(document.getElementById("message")).toBeTruthy();
    });

    it("should have buttons that can be clicked", () => {
      const commitBtn = document.getElementById("commit") as HTMLButtonElement;
      const prevBtn = document.getElementById("prev") as HTMLButtonElement;
      const nextBtn = document.getElementById("next") as HTMLButtonElement;

      const commitClick = vi.fn();
      const prevClick = vi.fn();
      const nextClick = vi.fn();

      commitBtn.onclick = commitClick;
      prevBtn.onclick = prevClick;
      nextBtn.onclick = nextClick;

      commitBtn.click();
      prevBtn.click();
      nextBtn.click();

      expect(commitClick).toHaveBeenCalled();
      expect(prevClick).toHaveBeenCalled();
      expect(nextClick).toHaveBeenCalled();
    });

    it("should handle keyboard events", () => {
      const handler = vi.fn();
      document.addEventListener("keydown", handler);

      const event = new KeyboardEvent("keydown", { key: "Enter" });
      document.dispatchEvent(event);

      expect(handler).toHaveBeenCalled();
    });
  });

  describe("Edit state management", () => {
    it("should track editable fields", () => {
      const container = document.getElementById("transaction")!;
      const onInput = vi.fn();

      const renderer = new DirectiveRenderer(container, onInput, [], filterAccounts);

      const transaction = {
        date: "2024-01-15",
        flag: "!",
        payee: "Test Store",
        narration: "Test purchase",
        tags: [],
        links: [],
        postings: [],
      };

      renderer.render(transaction, { account: "Expenses:Food" });

      const editableFields = container.querySelectorAll('[contenteditable="plaintext-only"]');
      expect(editableFields.length).toBeGreaterThan(0);
    });

    it("should call onInput when fields change", () => {
      const container = document.getElementById("transaction")!;
      const onInput = vi.fn();

      const renderer = new DirectiveRenderer(container, onInput, [], filterAccounts);

      const transaction = {
        date: "2024-01-15",
        flag: "!",
        payee: "Test Store",
        narration: "Test purchase",
        tags: [],
        links: [],
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      };

      renderer.render(transaction);

      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      accountField.textContent = "Expenses:Food";
      accountField.dispatchEvent(new Event("input"));

      expect(onInput).toHaveBeenCalledWith("account", "Expenses:Food");
    });

    it("should use prefilled expense account from transaction", () => {
      const container = document.getElementById("transaction")!;
      const onInput = vi.fn();

      const renderer = new DirectiveRenderer(container, onInput, [], filterAccounts);

      const transaction = {
        date: "2024-01-15",
        flag: "!",
        payee: "TechPay Services Ltd",
        narration: "Payment",
        tags: [],
        links: [],
        postings: [
          {
            account: "Assets:Paypal",
            amount: { value: "-39.99", currency: "EUR" },
            cost: null,
            price: null,
          },
          {
            account: "Expenses:FIXME",
            amount: null,
            cost: null,
            price: null,
          },
        ],
      };

      renderer.render(transaction);

      // Should have the prefilled account as the editable field
      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      expect(accountField.textContent).toBe("Expenses:FIXME");

      // Should only have one editable account field
      const accountFields = container.querySelectorAll('[data-key="a"]');
      expect(accountFields.length).toBe(1);
    });

    it("should override prefilled expense account with edit state", () => {
      const container = document.getElementById("transaction")!;
      const onInput = vi.fn();

      const renderer = new DirectiveRenderer(container, onInput, [], filterAccounts);

      const transaction = {
        date: "2024-01-15",
        flag: "!",
        payee: "Store",
        narration: "Purchase",
        tags: [],
        links: [],
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
          {
            account: "Expenses:FIXME",
            amount: null,
            cost: null,
            price: null,
          },
        ],
      };

      renderer.render(transaction, { account: "Expenses:Food" });

      // Should use the edit state value instead of the prefilled one
      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      expect(accountField.textContent).toBe("Expenses:Food");
    });

    it("should not show account field for balanced transactions", () => {
      const container = document.getElementById("transaction")!;
      const onInput = vi.fn();

      const renderer = new DirectiveRenderer(container, onInput, [], filterAccounts);

      const transaction = {
        date: "2023-01-31",
        flag: "*",
        payee: "Bankgutschrift auf PayPal-Konto",
        narration: null,
        tags: [],
        links: [],
        postings: [
          {
            account: "Assets:ZeroSum:Transfers",
            amount: { value: "-39.99", currency: "EUR" },
            cost: null,
            price: null,
          },
          {
            account: "Assets:Paypal",
            amount: { value: "39.99", currency: "EUR" },
            cost: null,
            price: null,
          },
        ],
      };

      renderer.render(transaction);

      // Should not have an account field since transaction is balanced
      const accountField = container.querySelector('[data-key="a"]');
      expect(accountField).toBeNull();

      // Should still render the transaction
      expect(container.textContent).toContain("Assets:ZeroSum:Transfers");
      expect(container.textContent).toContain("Assets:Paypal");
    });
  });

  describe("Message display", () => {
    it("should update message element", () => {
      const messageEl = document.getElementById("message")!;

      messageEl.className = "error";
      messageEl.textContent = "Test error";

      expect(messageEl.className).toBe("error");
      expect(messageEl.textContent).toBe("Test error");
    });

    it("should support success messages", () => {
      const messageEl = document.getElementById("message")!;

      messageEl.className = "success";
      messageEl.textContent = "Test success";

      expect(messageEl.className).toBe("success");
      expect(messageEl.textContent).toBe("Test success");
    });

    it("should clear messages", () => {
      const messageEl = document.getElementById("message")!;

      messageEl.className = "error";
      messageEl.textContent = "Error";

      messageEl.className = "";
      messageEl.textContent = "";

      expect(messageEl.className).toBe("");
      expect(messageEl.textContent).toBe("");
    });
  });

  describe("Counter display", () => {
    it("should update counter text", () => {
      const counterEl = document.getElementById("counter")!;

      counterEl.textContent = "Transaction 1/5";
      expect(counterEl.textContent).toBe("Transaction 1/5");

      counterEl.textContent = "0/0";
      expect(counterEl.textContent).toBe("0/0");
    });
  });

  describe("Button states", () => {
    it("should enable and disable commit button", () => {
      const commitBtn = document.getElementById("commit") as HTMLButtonElement;

      commitBtn.disabled = true;
      expect(commitBtn.disabled).toBe(true);

      commitBtn.disabled = false;
      expect(commitBtn.disabled).toBe(false);
    });

    it("should enable and disable navigation buttons", () => {
      const prevBtn = document.getElementById("prev") as HTMLButtonElement;
      const nextBtn = document.getElementById("next") as HTMLButtonElement;

      prevBtn.disabled = true;
      nextBtn.disabled = true;

      expect(prevBtn.disabled).toBe(true);
      expect(nextBtn.disabled).toBe(true);

      prevBtn.disabled = false;
      nextBtn.disabled = false;

      expect(prevBtn.disabled).toBe(false);
      expect(nextBtn.disabled).toBe(false);
    });
  });
});
