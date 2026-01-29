import { describe, it, expect, beforeEach, vi, type Mock } from "vitest";
import { DirectiveRenderer } from "../src/directive-renderer";
import type { Transaction } from "../src/model/beancount";

describe("DirectiveRenderer", () => {
  let container: HTMLElement;
  let onInputMock: Mock<(field: "payee" | "narration" | "account", value: string) => void>;
  let filterMock: Mock<(query: string, items: string[]) => string[]>;
  let renderer: DirectiveRenderer;

  beforeEach(() => {
    document.body.innerHTML = '<div id="test-container"></div>';
    container = document.getElementById("test-container")!;
    onInputMock = vi.fn();
    filterMock = vi.fn((query: string, items: string[]) => items);

    renderer = new DirectiveRenderer(
      container,
      onInputMock,
      ["Expenses:Food", "Expenses:Travel"],
      filterMock,
    );
  });

  const createTransaction = (overrides?: Partial<Transaction>): Transaction => ({
    date: "2024-01-15",
    flag: "!",
    payee: "Test Store",
    narration: "Test purchase",
    tags: [],
    links: [],
    postings: [
      {
        account: "Assets:Bank:Checking",
        amount: { value: "-50.00", currency: "USD" },
        cost: null,
        price: null,
      },
    ],
    ...overrides,
  });

  describe("render", () => {
    it("should render basic transaction", () => {
      const txn = createTransaction();
      renderer.render(txn);

      expect(container.textContent).toContain("2024-01-15");
      expect(container.textContent).toContain("!");
      expect(container.textContent).toContain("Test Store");
      expect(container.textContent).toContain("Test purchase");
      expect(container.textContent).toContain("Assets:Bank:Checking");
      expect(container.textContent).toContain("-50.00");
      expect(container.textContent).toContain("USD");
    });

    it("should render transaction with null payee", () => {
      const txn = createTransaction({ payee: null });
      renderer.render(txn);

      expect(container.textContent).toContain("2024-01-15");
      expect(container.textContent).toContain("Test purchase");
      expect(container.textContent).not.toContain("Test Store");
    });

    it("should render transaction with null narration", () => {
      const txn = createTransaction({ narration: null });
      renderer.render(txn);

      expect(container.textContent).toContain("2024-01-15");
      expect(container.textContent).toContain("Test Store");
    });

    it("should render transaction with tags", () => {
      const txn = createTransaction({ tags: ["vacation", "personal"] });
      renderer.render(txn);

      expect(container.textContent).toContain("#vacation");
      expect(container.textContent).toContain("#personal");
    });

    it("should render transaction with links", () => {
      const txn = createTransaction({ links: ["invoice-123", "receipt-456"] });
      renderer.render(txn);

      expect(container.textContent).toContain("^invoice-123");
      expect(container.textContent).toContain("^receipt-456");
    });

    it("should render multiple postings", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank:Checking",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
          {
            account: "Expenses:Food",
            amount: { value: "50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      });
      renderer.render(txn);

      expect(container.textContent).toContain("Assets:Bank:Checking");
      expect(container.textContent).toContain("Expenses:Food");
    });

    it("should render posting with cost", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Stocks",
            amount: { value: "10", currency: "AAPL" },
            cost: "150.00 USD",
            price: null,
          },
        ],
      });
      renderer.render(txn);

      expect(container.textContent).toContain("150.00 USD");
    });

    it("should render posting with price", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Stocks",
            amount: { value: "10", currency: "AAPL" },
            cost: null,
            price: "150.00 USD",
          },
        ],
      });
      renderer.render(txn);

      expect(container.textContent).toContain("@");
      expect(container.textContent).toContain("150.00 USD");
    });

    it("should render editable account field with edit state for unbalanced transaction", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank:Checking",
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
      });
      renderer.render(txn, { account: "Expenses:Food" });

      const editableFields = container.querySelectorAll('[contenteditable="plaintext-only"]');
      const accountField = Array.from(editableFields).find(
        (el) => el.textContent === "Expenses:Food",
      );
      expect(accountField).toBeTruthy();
    });

    it("should use edit state for payee", () => {
      const txn = createTransaction({ payee: "Original" });
      renderer.render(txn, { account: "", payee: "Updated Payee" });

      expect(container.textContent).toContain("Updated Payee");
      expect(container.textContent).not.toContain("Original");
    });

    it("should use edit state for narration", () => {
      const txn = createTransaction({ narration: "Original" });
      renderer.render(txn, { account: "", narration: "Updated Narration" });

      expect(container.textContent).toContain("Updated Narration");
      expect(container.textContent).not.toContain("Original");
    });

    it("should create editable fields with data-key attributes for unbalanced transaction", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank:Checking",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      });
      renderer.render(txn);

      const payeeField = container.querySelector('[data-key="p"]');
      expect(payeeField).toBeTruthy();

      const narrationField = container.querySelector('[data-key="n"]');
      expect(narrationField).toBeTruthy();

      const accountField = container.querySelector('[data-key="a"]');
      expect(accountField).toBeTruthy();
    });

    it("should use prefilled expense account when posting without amount exists", () => {
      const txn = createTransaction({
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
      });
      renderer.render(txn);

      // Should render the expense account as editable
      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      expect(accountField).toBeTruthy();
      expect(accountField.textContent).toBe("Expenses:FIXME");

      // Should only have one editable account field
      const editableAccountFields = container.querySelectorAll('[data-key="a"]');
      expect(editableAccountFields.length).toBe(1);
    });

    it("should use edit state for prefilled expense account", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-100.00", currency: "USD" },
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
      });
      renderer.render(txn, { account: "Expenses:Food" });

      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      expect(accountField).toBeTruthy();
      expect(accountField.textContent).toBe("Expenses:Food");
    });

    it("should show empty account field when no prefilled expense exists", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      });
      renderer.render(txn);

      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      expect(accountField).toBeTruthy();
      expect(accountField.textContent).toBe("");
    });

    it("should not show editable account field for balanced transactions", () => {
      const txn = createTransaction({
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
      });
      renderer.render(txn);

      // Should not have any editable account field
      const accountField = container.querySelector('[data-key="a"]');
      expect(accountField).toBeNull();

      // But should still show the transaction
      expect(container.textContent).toContain("Assets:ZeroSum:Transfers");
      expect(container.textContent).toContain("Assets:Paypal");
      expect(container.textContent).toContain("-39.99");
      expect(container.textContent).toContain("39.99");
    });

    it("should show editable account field for unbalanced transaction with all amounts", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
          {
            account: "Assets:Paypal",
            amount: { value: "30.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      });
      renderer.render(txn);

      // Should have an editable account field since totals don't sum to zero
      const accountField = container.querySelector('[data-key="a"]');
      expect(accountField).toBeTruthy();
    });
  });

  describe("editable field interactions", () => {
    it("should call onInput when payee is edited", () => {
      const txn = createTransaction();
      renderer.render(txn);

      const payeeField = container.querySelector('[data-key="p"]') as HTMLElement;
      payeeField.textContent = "New Payee";
      payeeField.dispatchEvent(new Event("input"));

      expect(onInputMock).toHaveBeenCalledWith("payee", "New Payee");
    });

    it("should call onInput when narration is edited", () => {
      const txn = createTransaction();
      renderer.render(txn);

      const narrationField = container.querySelector('[data-key="n"]') as HTMLElement;
      narrationField.textContent = "New Narration";
      narrationField.dispatchEvent(new Event("input"));

      expect(onInputMock).toHaveBeenCalledWith("narration", "New Narration");
    });

    it("should call onInput when account is edited", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank:Checking",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      });
      renderer.render(txn);

      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      accountField.textContent = "Expenses:Travel";
      accountField.dispatchEvent(new Event("input"));

      expect(onInputMock).toHaveBeenCalledWith("account", "Expenses:Travel");
    });

    it("should select all text on focus", () => {
      const txn = createTransaction();
      renderer.render(txn);

      const payeeField = container.querySelector('[data-key="p"]') as HTMLElement;

      // Mock selection
      const range = document.createRange();
      const selectNodeContentsSpy = vi.spyOn(range, "selectNodeContents");
      vi.spyOn(document, "createRange").mockReturnValue(range);

      payeeField.dispatchEvent(new Event("focus"));

      expect(selectNodeContentsSpy).toHaveBeenCalledWith(payeeField);
    });

    it("should blur on Escape key", () => {
      const txn = createTransaction();
      renderer.render(txn);

      const payeeField = container.querySelector('[data-key="p"]') as HTMLElement;
      const blurSpy = vi.spyOn(payeeField, "blur");

      const event = new KeyboardEvent("keydown", { key: "Escape", bubbles: true });
      payeeField.dispatchEvent(event);

      expect(blurSpy).toHaveBeenCalled();
    });

    it("should blur on Enter key", () => {
      const txn = createTransaction();
      renderer.render(txn);

      const narrationField = container.querySelector('[data-key="n"]') as HTMLElement;
      const blurSpy = vi.spyOn(narrationField, "blur");

      const event = new KeyboardEvent("keydown", { key: "Enter", bubbles: true });
      narrationField.dispatchEvent(event);

      expect(blurSpy).toHaveBeenCalled();
    });

    it("should stop propagation of keyboard events", () => {
      const txn = createTransaction();
      renderer.render(txn);

      const payeeField = container.querySelector('[data-key="p"]') as HTMLElement;

      const event = new KeyboardEvent("keydown", { key: "h", bubbles: true });
      const stopPropSpy = vi.spyOn(event, "stopPropagation");

      payeeField.dispatchEvent(event);

      expect(stopPropSpy).toHaveBeenCalled();
    });
  });

  describe("account field autocomplete", () => {
    it("should trigger autocomplete on input", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      });
      renderer.render(txn, { account: "Exp" });

      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      accountField.textContent = "Expenses";
      accountField.dispatchEvent(new Event("input"));

      expect(onInputMock).toHaveBeenCalledWith("account", "Expenses");
    });

    it("should handle Tab for autocomplete navigation", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      });
      renderer.render(txn);

      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;

      const tabEvent = new KeyboardEvent("keydown", { key: "Tab", bubbles: true });
      const preventDefaultSpy = vi.spyOn(tabEvent, "preventDefault");

      // Trigger focus to show autocomplete
      accountField.dispatchEvent(new Event("focus"));
      accountField.dispatchEvent(tabEvent);

      expect(preventDefaultSpy).toHaveBeenCalled();
    });

    it("should handle ArrowDown for autocomplete", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      });
      renderer.render(txn);

      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      accountField.dispatchEvent(new Event("focus"));

      const arrowEvent = new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true });
      const preventDefaultSpy = vi.spyOn(arrowEvent, "preventDefault");

      accountField.dispatchEvent(arrowEvent);

      expect(preventDefaultSpy).toHaveBeenCalled();
    });
  });

  describe("setAvailableAccounts", () => {
    it("should update autocomplete items", () => {
      const newAccounts = ["Income:Salary", "Expenses:Rent"];
      renderer.setAvailableAccounts(newAccounts);

      // This test mainly verifies no errors are thrown
      // The actual autocomplete behavior is tested in autocomplete.test.ts
      const txn = createTransaction();
      renderer.render(txn);
      expect(container).toBeTruthy();
    });
  });

  describe("keyboard shortcuts", () => {
    it("should focus payee field on 'p' key", () => {
      const txn = createTransaction();
      renderer.render(txn);

      const payeeField = container.querySelector('[data-key="p"]') as HTMLElement;
      const focusSpy = vi.spyOn(payeeField, "focus");

      const event = new KeyboardEvent("keydown", { key: "p", bubbles: true });
      document.dispatchEvent(event);

      expect(focusSpy).toHaveBeenCalled();
    });

    it("should focus narration field on 'n' key", () => {
      const txn = createTransaction();
      renderer.render(txn);

      const narrationField = container.querySelector('[data-key="n"]') as HTMLElement;
      const focusSpy = vi.spyOn(narrationField, "focus");

      const event = new KeyboardEvent("keydown", { key: "n", bubbles: true });
      document.dispatchEvent(event);

      expect(focusSpy).toHaveBeenCalled();
    });

    it("should focus account field on 'a' key", () => {
      const txn = createTransaction({
        postings: [
          {
            account: "Assets:Bank",
            amount: { value: "-50.00", currency: "USD" },
            cost: null,
            price: null,
          },
        ],
      });
      renderer.render(txn);

      const accountField = container.querySelector('[data-key="a"]') as HTMLElement;
      const focusSpy = vi.spyOn(accountField, "focus");

      const event = new KeyboardEvent("keydown", { key: "a", bubbles: true });
      document.dispatchEvent(event);

      expect(focusSpy).toHaveBeenCalled();
    });
  });
});
