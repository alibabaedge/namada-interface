import { Chain } from "@chain-registry/types";
import { Panel } from "@namada/components";
import { AccountType } from "@namada/types";
import { Timeline } from "App/Common/Timeline";
import { params } from "App/routes";
import {
  OnSubmitTransferParams,
  TransactionFee,
  TransferModule,
} from "App/Transfer/TransferModule";
import { allDefaultAccountsAtom } from "atoms/accounts";
import { namadaShieldedAssetsAtom } from "atoms/balance/atoms";
import { chainParametersAtom } from "atoms/chain/atoms";
import { defaultGasConfigFamily } from "atoms/fees/atoms";
import { unshieldTxAtom } from "atoms/shield/atoms";
import BigNumber from "bignumber.js";
import clsx from "clsx";
import { AnimatePresence, motion } from "framer-motion";
import { useTransactionActions } from "hooks/useTransactionActions";
import { wallets } from "integrations";
import { getAssetImageUrl } from "integrations/utils";
import { useAtomValue } from "jotai";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import namadaChain from "registry/namada.json";
import {
  Address,
  PartialTransferTransactionData,
  TransferStep,
  TransferTransactionData,
} from "types";
import { MaspTopHeader } from "./MaspTopHeader";

export const MaspUnshield: React.FC = () => {
  const [searchParams, setSearchParams] = useSearchParams();

  const chainParameters = useAtomValue(chainParametersAtom);
  const defaultAccounts = useAtomValue(allDefaultAccountsAtom);
  const { data: availableAssets, isLoading: isLoadingAssets } = useAtomValue(
    namadaShieldedAssetsAtom
  );
  const performUnshieldTransfer = useAtomValue(unshieldTxAtom);

  const [amount, setAmount] = useState<BigNumber | undefined>();
  const [currentStep, setCurrentStep] = useState(0);
  const [generalErrorMessage, setGeneralErrorMessage] = useState("");

  const [transaction, setTransaction] =
    useState<PartialTransferTransactionData>();

  const {
    transactions: myTransactions,
    findByHash,
    storeTransaction,
  } = useTransactionActions();

  const chainId = chainParameters.data?.chainId;

  const sourceAddress = defaultAccounts.data?.find(
    (account) => account.type === AccountType.ShieldedKeys
  )?.address;
  const destinationAddress = defaultAccounts.data?.find(
    (account) => account.type !== AccountType.ShieldedKeys
  )?.address;

  const selectedAssetAddress = searchParams.get(params.asset) || undefined;
  const selectedAsset =
    selectedAssetAddress ? availableAssets?.[selectedAssetAddress] : undefined;

  const { data: gasConfig } = useAtomValue(
    defaultGasConfigFamily(["UnshieldingTransfer"])
  );

  const transactionFee: TransactionFee | undefined =
    selectedAsset && gasConfig ?
      {
        originalAddress: selectedAsset.originalAddress,
        asset: selectedAsset.asset,
        amount: gasConfig.gasPrice.multipliedBy(gasConfig.gasLimit),
      }
    : undefined;

  const assetImage = selectedAsset ? getAssetImageUrl(selectedAsset.asset) : "";

  useEffect(() => {
    if (transaction?.hash) {
      const tx = findByHash(transaction.hash);
      if (tx) {
        setTransaction(tx);
      }
    }
  }, [myTransactions]);

  const onChangeSelectedAsset = (address?: Address): void => {
    setSearchParams(
      (currentParams) => {
        const newParams = new URLSearchParams(currentParams);
        if (address) {
          newParams.set(params.asset, address);
        } else {
          newParams.delete(params.asset);
        }
        return newParams;
      },
      { replace: false }
    );
  };

  const onSubmitTransfer = async ({
    displayAmount,
    destinationAddress,
  }: OnSubmitTransferParams): Promise<void> => {
    try {
      setGeneralErrorMessage("");
      setCurrentStep(1);

      if (typeof sourceAddress === "undefined") {
        throw new Error("Source address is not defined");
      }

      if (!chainId) {
        throw new Error("Chain ID is undefined");
      }

      if (!selectedAsset) {
        throw new Error("No asset is selected");
      }

      if (typeof gasConfig === "undefined") {
        throw new Error("No gas config");
      }

      setTransaction({
        type: "ShieldedToTransparent",
        asset: selectedAsset.asset,
        chainId,
        currentStep: TransferStep.Sign,
      });

      const txResponse = await performUnshieldTransfer.mutateAsync({
        sourceAddress,
        destinationAddress,
        tokenAddress: selectedAsset.originalAddress,
        amount: displayAmount,
        gasConfig,
      });

      // TODO review and improve this data to be more precise and full of details
      const tx: TransferTransactionData = {
        type: "ShieldedToTransparent",
        currentStep: TransferStep.Complete,
        sourceAddress,
        destinationAddress,
        asset: selectedAsset.asset,
        displayAmount,
        rpc: txResponse.msg.payload.chain.rpcUrl,
        chainId: txResponse.msg.payload.chain.chainId,
        hash: txResponse.encodedTx.txs[0]?.hash,
        feePaid: txResponse.encodedTx.wrapperTxProps.feeAmount,
        resultTxHash: txResponse.encodedTx.txs[0]?.innerTxHashes[0],
        status: "success",
        createdAt: new Date(),
        updatedAt: new Date(),
      };
      setTransaction(tx);
      storeTransaction(tx);

      setCurrentStep(2);
    } catch (err) {
      setGeneralErrorMessage(err + "");
      setCurrentStep(0);
    }
  };

  return (
    <Panel className="pt-8 pb-20">
      <header className="flex flex-col items-center text-center mb-3 gap-6">
        <h1 className="text-lg">Unshield</h1>
        <MaspTopHeader type="unshield" isShielded />
        <h2 className="text-lg">Namada Shielded to Namada Transparent</h2>
      </header>
      <AnimatePresence>
        {currentStep === 0 && (
          <motion.div
            key="transfer"
            exit={{ opacity: 0 }}
            className="min-h-[600px]"
          >
            <TransferModule
              source={{
                isLoadingAssets: isLoadingAssets,
                availableAssets,
                selectedAssetAddress,
                availableAmount: selectedAsset?.amount,
                chain: namadaChain as Chain,
                availableWallets: [wallets.namada!],
                wallet: wallets.namada,
                walletAddress: sourceAddress,
                isShielded: true,
                onChangeSelectedAsset,
                amount,
                onChangeAmount: setAmount,
              }}
              destination={{
                chain: namadaChain as Chain,
                availableWallets: [wallets.namada!],
                wallet: wallets.namada,
                walletAddress: destinationAddress,
                isShielded: false,
              }}
              transactionFee={transactionFee}
              isSubmitting={performUnshieldTransfer.isPending}
              errorMessage={generalErrorMessage}
              onSubmitTransfer={onSubmitTransfer}
            />
          </motion.div>
        )}
        {currentStep > 0 && (
          <motion.div
            key="progress"
            className={clsx("my-12 ")}
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
          >
            <Timeline
              currentStepIndex={currentStep}
              steps={[
                {
                  children: <img src={assetImage} className="w-14" />,
                },
                { children: "Signature Required", bullet: true },
                { children: "Asset Leaving MASP" },
                {
                  children: (
                    <>
                      <img src={assetImage} className="w-14 mb-2" />
                      Unshielded Transfer Complete
                    </>
                  ),
                },
              ]}
            />
          </motion.div>
        )}
      </AnimatePresence>
    </Panel>
  );
};
