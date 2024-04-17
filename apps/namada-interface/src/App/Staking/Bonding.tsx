import {
  ActionButton,
  Heading,
  Modal,
  Panel,
  ProgressIndicator,
} from "@namada/components";
import { ModalContainer } from "App/Common/ModalContainer";
import BigNumber from "bignumber.js";
import { useAtomValue } from "jotai";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { totalNamBalanceAtom } from "slices/accounts";
import { selectedCurrencyRateAtom } from "slices/exchangeRates";
import { selectedCurrencyAtom } from "slices/settings";
import { getStakingTotalAtom } from "slices/staking";
import {
  Validator,
  fetchAllValidatorsAtom,
  fetchMyValidatorsAtom,
} from "slices/validators";
import { useLoadable } from "store/hooks";
import { BondingAmountOverview } from "./BondingAmountOverview";
import { BondingValidatorsTable } from "./BondingValidatorsTable";
import { ValidatorSearch } from "./ValidatorSearch";
import StakingRoutes from "./routes";

type ValidatorAddress = string;

export const Bonding = (): JSX.Element => {
  const [filter, setFilter] = useState<string>("");
  const navigate = useNavigate();
  const totalNam = useAtomValue(totalNamBalanceAtom);
  const selectedFiatCurrency = useAtomValue(selectedCurrencyAtom);
  const selectedCurrencyRate = useAtomValue(selectedCurrencyRateAtom);
  const validators = useLoadable(fetchAllValidatorsAtom);
  const totalStakedValue = useAtomValue(getStakingTotalAtom);
  const myValidators = useLoadable(fetchMyValidatorsAtom);

  const [selectedValidators, setSelectedValidators] = useState<
    Record<ValidatorAddress, boolean>
  >({});

  const [stakedAmounts, setStakedAmounts] = useState<
    Record<ValidatorAddress, BigNumber>
  >({});

  const [newAmountsToStake, setNewAmountsToStake] = useState<
    Record<ValidatorAddress, BigNumber>
  >({});

  useEffect(() => {
    if (myValidators.state !== "hasData") return;
    const stakedAmounts: Record<ValidatorAddress, BigNumber> = {};
    const selectedValidators: Record<ValidatorAddress, boolean> = {};
    for (const myValidator of myValidators.data) {
      stakedAmounts[myValidator.validator.address] =
        myValidator.stakedAmount || new BigNumber(0);
      selectedValidators[myValidator.validator.address] = true;
    }
    setStakedAmounts(stakedAmounts);
    setSelectedValidators(selectedValidators);
  }, [myValidators.state]);

  const onChangeValidatorsAmount = (
    validator: Validator,
    amount: BigNumber
  ): void => {
    onAddValidator(validator);
    setNewAmountsToStake((obj) => ({
      ...obj,
      [validator.address]: amount,
    }));
  };

  const onAddValidator = (validator: Validator): void => {
    setSelectedValidators((obj) => ({
      ...obj,
      [validator.address]: true,
    }));
  };

  const onRemoveValidator = (validator: Validator): void => {
    setNewAmountsToStake((obj) => {
      const { [validator.address]: _, ...validators } = obj;
      return validators;
    });

    setSelectedValidators((obj) => ({
      ...obj,
      [validator.address]: false,
    }));
  };

  const onClose = (): void => navigate(StakingRoutes.overview().url);

  const header = (
    <>
      <div className="left-0 absolute">
        <ProgressIndicator
          keyName="bonding-steps"
          totalSteps={2}
          currentStep={1}
        />
      </div>
      <Heading>Select Validators to delegate your NAM</Heading>
    </>
  );

  return (
    <Modal onClose={onClose}>
      <ModalContainer header={header} onClose={onClose}>
        <div className="flex gap-2">
          <BondingAmountOverview
            title="Available NAM to Stake"
            selectedFiatCurrency={selectedFiatCurrency}
            fiatExchangeRate={selectedCurrencyRate}
            amountInNam={totalNam}
            amountInFiat={totalNam.multipliedBy(selectedCurrencyRate)}
          />
          <BondingAmountOverview
            title="Current Staked Amount"
            selectedFiatCurrency={selectedFiatCurrency}
            fiatExchangeRate={selectedCurrencyRate}
            amountInNam={totalStakedValue.totalBonded}
            amountInFiat={totalStakedValue.totalBonded.multipliedBy(
              selectedCurrencyRate
            )}
          />
        </div>
        <Panel className="w-full rounded-md flex-1">
          <div className="w-[70%]">
            <ValidatorSearch onChange={(value: string) => setFilter(value)} />
          </div>
          {validators.state === "hasData" && (
            <BondingValidatorsTable
              filter={filter}
              validators={validators.data}
              selectedValidators={selectedValidators}
              stakedAmountsByValidator={stakedAmounts}
              newStakedAmountsByValidator={newAmountsToStake}
              onChangeAmount={onChangeValidatorsAmount}
              onAddValidator={onAddValidator}
              onRemoveValidator={onRemoveValidator}
            />
          )}
        </Panel>
        <ActionButton
          size="sm"
          borderRadius="sm"
          className="mt-2 w-1/4 mx-auto"
        >
          Stake
        </ActionButton>
      </ModalContainer>
    </Modal>
  );
};
