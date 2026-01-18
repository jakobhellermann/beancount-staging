import { describe, it, expect, beforeEach, vi } from "vitest";
import { ApiClient } from "../src/api";

describe("ApiClient", () => {
  let client: ApiClient;

  beforeEach(() => {
    client = new ApiClient();
    // Reset fetch mock before each test
    global.fetch = vi.fn();
  });

  describe("init", () => {
    it("should fetch initial data successfully", async () => {
      const mockData = {
        items: [],
        current_index: 0,
        available_accounts: ["Expenses:Food", "Expenses:Travel"],
      };

      global.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: async () => mockData,
      });

      const result = await client.init();

      expect(global.fetch).toHaveBeenCalledWith("/api/init");
      expect(result).toEqual(mockData);
    });

    it("should throw error on failed request", async () => {
      global.fetch = vi.fn().mockResolvedValue({
        ok: false,
        statusText: "Internal Server Error",
      });

      await expect(client.init()).rejects.toThrow("Failed to initialize: Internal Server Error");
    });
  });

  describe("getTransaction", () => {
    it("should fetch transaction by id", async () => {
      const mockTransaction = {
        transaction: {
          id: "txn-1",
          transaction: {
            date: "2024-01-01",
            flag: "!",
            payee: "Test Store",
            narration: "Test purchase",
            tags: [],
            links: [],
            postings: [],
          },
        },
      };

      global.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: async () => mockTransaction,
      });

      const result = await client.getTransaction("txn-1");

      expect(global.fetch).toHaveBeenCalledWith("/api/transaction/txn-1");
      expect(result).toEqual(mockTransaction);
    });

    it("should throw error on failed request", async () => {
      global.fetch = vi.fn().mockResolvedValue({
        ok: false,
        statusText: "Not Found",
      });

      await expect(client.getTransaction("invalid-id")).rejects.toThrow(
        "Failed to load transaction: Not Found",
      );
    });
  });

  describe("commitTransaction", () => {
    it("should commit transaction successfully", async () => {
      const mockResponse = {
        ok: true,
        remaining_count: 5,
      };

      global.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: async () => mockResponse,
      });

      const result = await client.commitTransaction("txn-1", { account: "Expenses:Food" });

      expect(global.fetch).toHaveBeenCalledWith("/api/transaction/txn-1/commit", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ account: "Expenses:Food" }),
      });
      expect(result).toEqual(mockResponse);
    });

    it("should throw error on failed request with error message", async () => {
      global.fetch = vi.fn().mockResolvedValue({
        ok: false,
        statusText: "Bad Request",
        json: async () => ({ error: "Invalid account" }),
      });

      await expect(client.commitTransaction("txn-1", "Invalid")).rejects.toThrow("Invalid account");
    });

    it("should throw error on failed request without error message", async () => {
      global.fetch = vi.fn().mockResolvedValue({
        ok: false,
        statusText: "Bad Request",
        json: async () => {
          throw new Error("Invalid JSON");
        },
      });

      await expect(client.commitTransaction("txn-1", "Invalid")).rejects.toThrow("Bad Request");
    });
  });
});
