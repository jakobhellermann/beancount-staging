export type Directive =
  | ({ id: string; type: "transaction" } & Transaction)
  | ({ id: string; type: "balance" } & Balance);

export interface Transaction {
  date: string;
  flag: string;
  payee: string | null;
  narration: string | null;
  tags: string[];
  links: string[];
  postings: Posting[];
}

export interface Balance {
  date: string;
  account: string;
  amount: Amount;
  tolerance: string | null;
}

export interface Posting {
  account: string;
  amount: Amount | null;
  cost: string | null;
  price: string | null;
}

export interface Amount {
  value: string;
  currency: string;
}
