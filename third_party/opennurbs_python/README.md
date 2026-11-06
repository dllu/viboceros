# OpenNURBS Python experiments

`local_zero_trial.py` is an altered Python adaptation of the public
`ON_LocalZero1::NewtonRaphson` routine from OpenNURBS at
`23fc677ba06e49212296ca75fab7fb6c2851b4ce`, under the [MIT license](LICENSE).
It records termination reasons and preserves the output-assignment edge case
for oracle experiments. Production curve construction does not use this trial.
