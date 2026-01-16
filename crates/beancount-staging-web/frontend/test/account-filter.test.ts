import { describe, it, expect } from "vitest";
import { filterAccounts } from "../src/account-filter";

describe("filterAccounts", () => {
  const accounts = [
    "Assets:Bank:Checking",
    "Assets:Bank:Savings",
    "Assets:Cash",
    "Expenses:Food:Groceries",
    "Expenses:Food:Restaurant",
    "Expenses:Transport:Car",
    "Expenses:Transport:Public",
    "Income:Salary",
    "Liabilities:CreditCard",
  ];

  describe("empty query", () => {
    it("should return all accounts when query is empty", () => {
      expect(filterAccounts("", accounts)).toEqual(accounts);
    });
  });

  describe("single part matching", () => {
    it("should filter by first part prefix", () => {
      const result = filterAccounts("exp", accounts);
      expect(result).toEqual([
        "Expenses:Food:Groceries",
        "Expenses:Food:Restaurant",
        "Expenses:Transport:Car",
        "Expenses:Transport:Public",
      ]);
    });

    it("should filter by middle part prefix", () => {
      const result = filterAccounts("food", accounts);
      expect(result).toEqual(["Expenses:Food:Groceries", "Expenses:Food:Restaurant"]);
    });

    it("should filter by last part prefix", () => {
      const result = filterAccounts("groc", accounts);
      expect(result).toEqual(["Expenses:Food:Groceries"]);
    });

    it("should be case insensitive", () => {
      const result = filterAccounts("EXPENSE", accounts);
      expect(result).toEqual([
        "Expenses:Food:Groceries",
        "Expenses:Food:Restaurant",
        "Expenses:Transport:Car",
        "Expenses:Transport:Public",
      ]);
    });
  });

  describe("multiple part matching", () => {
    it("should match multiple parts in order", () => {
      const result = filterAccounts("exp:food", accounts);
      expect(result).toEqual(["Expenses:Food:Groceries", "Expenses:Food:Restaurant"]);
    });

    it("should match multiple parts out of order", () => {
      const result = filterAccounts("food:exp", accounts);
      expect(result).toEqual(["Expenses:Food:Groceries", "Expenses:Food:Restaurant"]);
    });

    it("should match three parts", () => {
      const result = filterAccounts("exp:food:groc", accounts);
      expect(result).toEqual(["Expenses:Food:Groceries"]);
    });

    it("should match abbreviated parts", () => {
      const result = filterAccounts("ass:bank", accounts);
      expect(result).toEqual(["Assets:Bank:Checking", "Assets:Bank:Savings"]);
    });

    it("should match very short abbreviations", () => {
      const result = filterAccounts("e:f", accounts);
      expect(result).toEqual(["Expenses:Food:Groceries", "Expenses:Food:Restaurant"]);
    });
  });

  describe("sorting", () => {
    it("should prioritize in-order matches", () => {
      const result = filterAccounts("exp:trans", accounts);
      // "Expenses:Transport:*" should come first as they match in order
      expect(result[0]).toBe("Expenses:Transport:Car");
      expect(result[1]).toBe("Expenses:Transport:Public");
    });

    it("should sort alphabetically when priority is equal", () => {
      const result = filterAccounts("bank", accounts);
      expect(result).toEqual(["Assets:Bank:Checking", "Assets:Bank:Savings"]);
    });

    it("should handle single matching account", () => {
      const result = filterAccounts("salary", accounts);
      expect(result).toEqual(["Income:Salary"]);
    });
  });

  describe("no matches", () => {
    it("should return empty array when no accounts match", () => {
      const result = filterAccounts("xyz", accounts);
      expect(result).toEqual([]);
    });

    it("should return empty array when parts don't all match", () => {
      const result = filterAccounts("exp:salary", accounts);
      expect(result).toEqual([]);
    });
  });

  describe("edge cases", () => {
    it("should handle trailing colon", () => {
      const result = filterAccounts("exp:", accounts);
      expect(result).toEqual([
        "Expenses:Food:Groceries",
        "Expenses:Food:Restaurant",
        "Expenses:Transport:Car",
        "Expenses:Transport:Public",
      ]);
    });

    it("should handle leading colon", () => {
      const result = filterAccounts(":exp", accounts);
      expect(result).toEqual([
        "Expenses:Food:Groceries",
        "Expenses:Food:Restaurant",
        "Expenses:Transport:Car",
        "Expenses:Transport:Public",
      ]);
    });

    it("should handle multiple colons", () => {
      const result = filterAccounts("exp::food", accounts);
      expect(result).toEqual(["Expenses:Food:Groceries", "Expenses:Food:Restaurant"]);
    });

    it("should match complete account name", () => {
      const result = filterAccounts("Assets:Bank:Checking", accounts);
      expect(result).toEqual(["Assets:Bank:Checking"]);
    });

    it("should handle single character parts", () => {
      const result = filterAccounts("a:b", accounts);
      expect(result).toEqual(["Assets:Bank:Checking", "Assets:Bank:Savings"]);
    });
  });

  describe("real world scenarios", () => {
    it("should quickly find groceries", () => {
      const result = filterAccounts("groc", accounts);
      expect(result).toContain("Expenses:Food:Groceries");
      expect(result.length).toBe(1);
    });

    it("should find transport expenses with abbreviation", () => {
      const result = filterAccounts("e:t", accounts);
      expect(result).toContain("Expenses:Transport:Car");
      expect(result).toContain("Expenses:Transport:Public");
    });

    it("should disambiguate between similar accounts", () => {
      const checking = filterAccounts("check", accounts);
      expect(checking).toEqual(["Assets:Bank:Checking"]);

      const savings = filterAccounts("sav", accounts);
      expect(savings).toEqual(["Assets:Bank:Savings"]);
    });
  });
});
