import { Balance } from "@anomaorg/namada-indexer-client";
import { Asset } from "@chain-registry/types";
import { defaultAccountAtom } from "atoms/accounts/atoms";
import { fetchAccountBalance } from "atoms/accounts/services";
import { indexerApiAtom } from "atoms/api";
import { nativeTokenAddressAtom } from "atoms/chain/atoms";
import { shouldUpdateBalanceAtom } from "atoms/etc";
import { queryDependentFn } from "atoms/utils";
import BigNumber from "bignumber.js";
import { atomWithQuery } from "jotai-tanstack-query";
import { unknownAsset } from "registry/unknownAsset";
import { findExpoent } from "utils/registry";
import { assetsByDenomAtom, denomByAddressAtom } from "./atoms";
import { fetchCoinPrices } from "./services";

export type TokenBalance = {
  denom: string;
  asset: Asset;
  balance: BigNumber;
  dollar?: BigNumber;
};

export const transparentBalanceAtom = atomWithQuery<Balance[]>((get) => {
  const enablePolling = get(shouldUpdateBalanceAtom);
  const defaultAccountQuery = get(defaultAccountAtom);
  const api = get(indexerApiAtom);
  const namTokenAddressQuery = get(nativeTokenAddressAtom);

  return {
    refetchInterval: enablePolling ? 1000 : false,
    queryKey: ["transparent-balance", defaultAccountQuery.data],
    ...queryDependentFn(async () => {
      const response = await fetchAccountBalance(api, defaultAccountQuery.data);
      // TODO
      // The indexer is returning as `namnam`, but the SDK is returning as `nam` for the same address.
      // We need to define a common pattern here, so we can share the same atoms.
      // Wor now, we are transforming the api returned value from `namnam` to `nam`.
      return response.map((item) =>
        item.tokenAddress === namTokenAddressQuery.data ?
          { ...item, balance: BigNumber(item.balance).shiftedBy(-6).toString() }
        : item
      );
    }, [defaultAccountQuery]),
  };
});

export const tokenPriceAtom = atomWithQuery((get) => {
  // TODO merge shielded and transparent
  // but do not use queryDependentFn so they don't block each other
  // release as soon as possible
  const transparentBalanceQuery = get(transparentBalanceAtom);
  const denomByAddressQuery = get(denomByAddressAtom);
  const assetsByDenom = get(assetsByDenomAtom);

  return {
    queryKey: [
      "token-price",
      transparentBalanceQuery.data,
      denomByAddressQuery.data,
    ],
    ...queryDependentFn(async () => {
      const denomById: Record<string, string> = {};
      const ids: string[] = [];
      transparentBalanceQuery.data?.forEach(({ tokenAddress }) => {
        const denom = denomByAddressQuery.data?.[tokenAddress];
        const id = denom && assetsByDenom[denom]?.coingecko_id;
        if (id) {
          denomById[id] = denom;
          ids.push(id);
        }
      });

      const pricesById = await fetchCoinPrices(ids);
      const pricesByDenom: Record<string, number> = {
        // TODO mock NAM price
        // nam: 1,
        // namnam: 1 / 1_000_000,
      };
      Object.entries(pricesById).forEach(([id, { usd }]) => {
        const denom = denomById[id];
        pricesByDenom[denom] = usd;
      });
      return pricesByDenom;
    }, [transparentBalanceQuery, denomByAddressQuery]),
  };
});

export const transparentTokensAtom = atomWithQuery<TokenBalance[]>((get) => {
  const transparentBalanceQuery = get(transparentBalanceAtom);
  const denomByAddressQuery = get(denomByAddressAtom);
  const assetsByDenom = get(assetsByDenomAtom);
  const tokenPriceQuery = get(tokenPriceAtom);

  return {
    queryKey: [
      "shielded-tokens",
      transparentBalanceQuery.data,
      denomByAddressQuery.data,
      tokenPriceQuery.data,
    ],
    ...queryDependentFn(async () => {
      if (!transparentBalanceQuery.data || !denomByAddressQuery.data) {
        return [];
      }

      return transparentBalanceQuery.data.map(({ tokenAddress, balance }) => {
        const denom =
          denomByAddressQuery.data[tokenAddress] ?? unknownAsset.display;
        const asset = assetsByDenom[denom] ?? unknownAsset;
        const display = asset.display;

        const expoentInput = findExpoent(asset, denom);
        const expoentOutput = findExpoent(asset, display);
        const expoent = expoentOutput - expoentInput;

        const balanceBigNumber = new BigNumber(balance).dividedBy(
          Math.pow(10, expoent)
        );

        const tokenPrice = tokenPriceQuery.data?.[denom];
        const dollar =
          tokenPrice ? balanceBigNumber.multipliedBy(tokenPrice) : undefined;

        return {
          denom,
          asset,
          balance: balanceBigNumber,
          dollar,
        };
      });
    }, [transparentBalanceQuery, tokenPriceQuery, denomByAddressQuery]),
  };
});
