import pandas as pd

from pandas import DataFrame
from pathlib import Path

def parse_duration_to_ms(duration: str) -> float:
    if duration.endswith("ms"):
        return float(duration[:-2])
    if duration.endswith("µs") or duration.endswith("us"):
        return float(duration[:-2]) / 1000
    if duration.endswith("ns"):
        return float(duration[:-2]) / 1_000_000
    if duration.endswith("s"):
        return float(duration[:-1]) * 1000
    
    raise ValueError(f"Unknown duration format: {duration}")

def calculate_accuracy(tp: int, fp: int, tn: int, fn: int) -> float:
    return (tp + tn) / (tp + tn + fp + fn) if (tp + tn + fp + fn) > 0 else 0

def calculate_precision(tp: int, fp: int, tn: int, fn: int) -> float:
    return tp / (tp + fp) if (tp + fp) > 0 else 0

def calculate_recall(tp: int, fp: int, tn: int, fn: int) -> float:
    return tp / (tp + fn) if (tp + fn) > 0 else 0

def calculate_f1_score(precision: float, recall: float) -> float:
    return 2 * (precision * recall) / (precision + recall) if (precision + recall) > 0 else 0 

def load_csv_files(dir: Path) -> dict[str, list[DataFrame]]:
    csv_files = {}
    for file_path in dir.iterdir():
        if file_path.is_dir() or file_path.suffix != ".csv":
            continue
        
        df = pd.read_csv(file_path, sep=",")
        df["runtime_normal"] = df["runtime_normal"].apply(parse_duration_to_ms)
        df["runtime_attestation"] = df["runtime_attestation"].apply(parse_duration_to_ms)
        df["average_inference_time"] = df["average_inference_time"].apply(parse_duration_to_ms)

        csv_files[file_path.stem] = df
    
    return csv_files

def calculate_runtime_overhead(df: DataFrame) -> dict[str, float]:
    
    df["runtime_diff"] = ((df["runtime_attestation"] - df["runtime_normal"]) / df["runtime_normal"]) * 100    
    mean_runtime_diff_ms = (df["runtime_attestation"] - df["runtime_normal"]).mean()

    return {
        "normal": df["runtime_normal"].mean(),
        "attestation": df["runtime_attestation"].mean(),
        "inference_time": df["average_inference_time"].mean(),
        "overhead_percent": df["runtime_diff"].mean(),
        "overhead_ms": mean_runtime_diff_ms
    }

def visualize_runtime_overhead(cnn_df: dict, vgg_df) -> DataFrame:
    program_cnn = []
    program_vgg = []
    overhead_cnn = []
    overhead_vgg = []
    inference_cnn = []
    inference_vgg = []

    for key, value in cnn_df.items():
        results = calculate_runtime_overhead(value)
        
        program_cnn.append(key)
        overhead_cnn.append(results["overhead_ms"])
        inference_cnn.append(results["inference_time"])
    
    for key, value in vgg_df.items():
        results = calculate_runtime_overhead(value)

        program_vgg.append(key)
        overhead_vgg.append(results["overhead_ms"])
        inference_vgg.append(results["inference_time"])

    if program_cnn != program_vgg:
        raise ValueError("Program lists between CNN and VGG differ — cannot align for chart.")

    chart_runtime = pd.DataFrame({
        "VGG-16": overhead_vgg,
        "Custom CNN": overhead_cnn
    }, index=program_cnn)

    chart_inference = pd.DataFrame({
        "VGG-16": inference_vgg,
        "Custom CNN": inference_cnn
    }, index=program_cnn)

    return {
        "runtime": chart_runtime,
        "inference": chart_inference
    }

def calculate_model_performance(dfs: dict):
    eval_metrics = {}

    for key, df in dfs.items():
        metrics = {}

        is_bad_input = df["input_type"] == "Bad"
        is_good_input = df["input_type"] == "Good"

        tn_states = df.loc[is_bad_input, "amount_bad_attestation_results"].sum()
        fp_states = df.loc[is_bad_input, "amount_good_attestation_results"].sum()
        tp_states = df.loc[is_good_input, "amount_good_attestation_results"].sum()
        fn_states = df.loc[is_good_input, "amount_bad_attestation_results"].sum()

        metrics["accuracy_states"] = calculate_accuracy(tp_states, fp_states, tn_states, fn_states)
        metrics["precision_states"] = calculate_precision(tp_states, fp_states, tn_states, fn_states)
        metrics["recall_states"] = calculate_recall(tp_states, fp_states, tn_states, fn_states)
        metrics["f1_score_states"] = calculate_f1_score(metrics["precision_states"], metrics["recall_states"])

        is_bad_verdict = df["final_attestation_verdict"] == "Bad"
        is_good_verdict = df["final_attestation_verdict"] == "Good"

        tn_verdict = len(df.loc[is_bad_input & is_bad_verdict, ["input_type", "final_attestation_verdict"]])
        fp_verdict = len(df.loc[is_bad_input & is_good_verdict, ["input_type", "final_attestation_verdict"]])
        tp_verdict = len(df.loc[is_good_input & is_good_verdict, ["input_type", "final_attestation_verdict"]])
        fn_verdict = len(df.loc[is_good_input & is_bad_verdict, ["input_type", "final_attestation_verdict"]])

        metrics["accuracy_verdict"] = calculate_accuracy(tp_verdict, fp_verdict, tn_verdict, fn_verdict)
        metrics["precision_verdict"] = calculate_precision(tp_verdict, fp_verdict, tn_verdict, fn_verdict)
        metrics["recall_verdict"] = calculate_recall(tp_verdict, fp_verdict, tn_verdict, fn_verdict)
        metrics["f1_score_verdict"] = calculate_f1_score(metrics["precision_verdict"], metrics["recall_verdict"])

        eval_metrics[key] = metrics
    
    return eval_metrics

def print_metrics(dfs: DataFrame):
    
    for program_name, df in dfs.items():
        print(f"{program_name}:")
        
        df["runtime_diff"] = ((df["runtime_attestation"] - df["runtime_normal"]) / df["runtime_normal"]) * 100
        mean_runtime_diff = df["runtime_diff"].mean()
        mean_runtime_diff_ms = (df["runtime_attestation"] - df["runtime_normal"]).mean()

        print(f"Average runtime increase:\n{mean_runtime_diff:.2f}%\n{mean_runtime_diff_ms:.2f}ms\n{(mean_runtime_diff_ms/1000):.2f}\n")

        df["total_amount_attestations"] = df["amount_good_attestation_results"] + df["amount_bad_attestation_results"]
        average_amount_attestations = df["total_amount_attestations"].mean()
        print(f"Average amount of attestations per run (floored): {int(average_amount_attestations)}")

        is_bad_input = df["input_type"] == "Bad"
        is_good_input = df["input_type"] == "Good"

        tn_states = df.loc[is_bad_input, "amount_bad_attestation_results"].sum()
        fp_states = df.loc[is_bad_input, "amount_good_attestation_results"].sum()
        tp_states = df.loc[is_good_input, "amount_good_attestation_results"].sum()
        fn_states = df.loc[is_good_input, "amount_bad_attestation_results"].sum()

        accuracy_states = calculate_accuracy(tp_states, fp_states, tn_states, fn_states)
        precision_states = calculate_precision(tp_states, fp_states, tn_states, fn_states)
        recall_states = calculate_recall(tp_states, fp_states, tn_states, fn_states)
        f1_score_states = calculate_f1_score(precision_states, recall_states)

        print(f"Accuracy States: {accuracy_states:.4f}")
        print(f"Precision States: {precision_states:.4f}")
        print(f"Recall States: {recall_states:.4f}")
        print(f"F1-Score States: {f1_score_states:.4f}")

        is_bad_verdict = df["final_attestation_verdict"] == "Bad"
        is_good_verdict = df["final_attestation_verdict"] == "Good"

        tn_verdict = len(df.loc[is_bad_input & is_bad_verdict, ["input_type", "final_attestation_verdict"]])
        fp_verdict = len(df.loc[is_bad_input & is_good_verdict, ["input_type", "final_attestation_verdict"]])
        tp_verdict = len(df.loc[is_good_input & is_good_verdict, ["input_type", "final_attestation_verdict"]])
        fn_verdict = len(df.loc[is_good_input & is_bad_verdict, ["input_type", "final_attestation_verdict"]])

        accuracy_verdict = calculate_accuracy(tp_verdict, fp_verdict, tn_verdict, fn_verdict)
        precision_verdict = calculate_precision(tp_verdict, fp_verdict, tn_verdict, fn_verdict)
        recall_verdict = calculate_recall(tp_verdict, fp_verdict, tn_verdict, fn_verdict)
        f1_score_verdict = calculate_f1_score(precision_verdict, recall_verdict)

        print(f"\nAccuracy Verdict: {accuracy_verdict:.4f}")
        print(f"Precision Verdict: {precision_verdict:.4f}")
        print(f"Recall Verdict: {recall_verdict:.4f}")
        print(f"F1-Score Verdict: {f1_score_verdict:.4f}\n\n")

