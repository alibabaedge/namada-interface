import {
  Account,
  BondProps,
  RedelegateProps,
  WithdrawProps,
} from "@namada/types";
import BigNumber from "bignumber.js";
import { invariant } from "framer-motion";
import { getSdkInstance } from "hooks";
import { atomWithMutation, atomWithQuery } from "jotai-tanstack-query";
import { TransactionPair, buildTxPair } from "lib/query";
import { queryDependentFn } from "store/utils";
import { GasConfig } from "types";
import { ChangeInStakingPosition, RedelegateChange } from "types/staking";
import { chainAtom } from "./chain";
import { MyValidator, myValidatorsAtom } from "./validators";

const {
  NAMADA_INTERFACE_NAMADA_TOKEN:
    nativeToken = "tnam1qxgfw7myv4dh0qna4hq0xdg6lx77fzl7dcem8h7e",
} = process.env;

type StakingTotals = {
  totalBonded: BigNumber;
  totalUnbonded: BigNumber;
  totalWithdrawable: BigNumber;
};

type ChangeInStakingProps = {
  account: Account;
  changes: ChangeInStakingPosition[];
  gasConfig: GasConfig;
};

type RedelegateChangesProps = {
  account: Account;
  changes: RedelegateChange[];
  gasConfig: GasConfig;
};

export const getStakingTotalAtom = atomWithQuery<StakingTotals>((get) => {
  const myValidators = get(myValidatorsAtom);
  return {
    queryKey: ["staking-totals", myValidators.dataUpdatedAt],
    ...queryDependentFn(async (): Promise<StakingTotals> => {
      const validatorsData = myValidators.data || [];

      const totalBonded = validatorsData.reduce(
        (acc: BigNumber, validator: MyValidator) =>
          acc.plus(validator.stakedAmount ?? 0),
        new BigNumber(0)
      );

      const totalUnbonded = validatorsData.reduce(
        (acc: BigNumber, validator: MyValidator) =>
          acc.plus(validator.unbondedAmount ?? 0),
        new BigNumber(0)
      );

      const totalWithdrawable = validatorsData.reduce(
        (acc: BigNumber, validator: MyValidator) =>
          acc.plus(validator.withdrawableAmount ?? 0),
        new BigNumber(0)
      );

      return { totalBonded, totalUnbonded, totalWithdrawable };
    }, [myValidators]),
  };
});

const getStakingChangesParams = (
  account: Account,
  changes: ChangeInStakingPosition[]
): BondProps[] => {
  const address = nativeToken;
  invariant(!!address, "Invalid currency address");
  return changes.map((change) => ({
    source: account.address,
    validator: change.validatorId,
    amount: change.amount,
    nativeToken: address!,
  }));
};

const getRedelegateChangeParams = (
  account: Account,
  changes: RedelegateChange[]
): RedelegateProps[] => {
  return changes.map((change: RedelegateChange) => ({
    owner: account.address,
    ...change,
  }));
};

export const createBondTxAtom = atomWithMutation((get) => {
  const chain = get(chainAtom);
  return {
    mutationKey: ["create-bonding-tx"],
    enabled: chain.isSuccess,
    mutationFn: async ({
      changes,
      gasConfig,
      account,
    }: ChangeInStakingProps): Promise<
      TransactionPair<BondProps>[] | undefined
    > => {
      try {
        const { tx } = await getSdkInstance();
        const bondProps = getStakingChangesParams(account, changes);
        const transactionPairs = await buildTxPair(
          account,
          gasConfig,
          chain.data!,
          bondProps,
          tx.buildBond,
          bondProps[0].source
        );
        return transactionPairs;
      } catch (err) {
        console.error(err);
        throw err;
      }
    },
  };
});

export const createUnbondTxAtom = atomWithMutation((get) => {
  const chain = get(chainAtom);
  return {
    mutationKey: ["creat-unbonding-tx"],
    enabled: chain.isSuccess,
    mutationFn: async ({
      changes,
      gasConfig,
      account,
    }: ChangeInStakingProps) => {
      try {
        const { tx } = await getSdkInstance();
        const unbondProps = getStakingChangesParams(account, changes);
        const transactionPairs = await buildTxPair(
          account,
          gasConfig,
          chain.data!,
          unbondProps,
          tx.buildUnbond,
          unbondProps[0].source
        );
        return transactionPairs;
      } catch (err) {
        console.error(err);
        throw err;
      }
    },
  };
});

export const createReDelegateTxAtom = atomWithMutation((get) => {
  const chain = get(chainAtom);
  return {
    mutationKey: ["create-redelegate-tx"],
    enabled: chain.isSuccess,
    mutationFn: async ({
      changes,
      gasConfig,
      account,
    }: RedelegateChangesProps) => {
      try {
        const { tx } = await getSdkInstance();
        const redelegateProps = getRedelegateChangeParams(account, changes);
        const transactionPairs = await buildTxPair(
          account,
          gasConfig,
          chain.data!,
          redelegateProps,
          tx.buildRedelegate,
          redelegateProps[0].owner
        );
        return transactionPairs;
      } catch (err) {
        console.error(err);
        throw err;
      }
    },
  };
});

export const createWithdrawTxAtom = atomWithMutation((get) => {
  const chain = get(chainAtom);
  return {
    mutationKey: ["create-withdraw-tx"],
    enabled: chain.isSuccess,
    mutationFn: async ({
      changes,
      gasConfig,
      account,
    }: ChangeInStakingProps): Promise<
      TransactionPair<WithdrawProps>[] | undefined
    > => {
      try {
        const chain = get(chainAtom);
        const { tx } = await getSdkInstance();
        const withdrawProps = getStakingChangesParams(account, changes);
        const transactionPairs = await buildTxPair(
          account,
          gasConfig,
          chain.data!,
          withdrawProps,
          tx.buildWithdraw,
          withdrawProps[0].source
        );
        return transactionPairs;
      } catch (err) {
        console.error(err);
        throw err;
      }
    },
  };
});
