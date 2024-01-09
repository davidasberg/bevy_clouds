import matplotlib.pyplot as plt
import numpy as np

# Assume `data` is a 2D list or array where each row corresponds to a dataset
# and each column corresponds to a phase function. For example:
data = [
    [20, 20.1, 19], # Dataset 1 average frame times for each phase function
    [21, 20, 18], # Dataset 2
    [19, 18, 20], # Dataset 3
    [20, 21, 19], # Dataset 4
    [22, 21, 20], # Dataset 5
    [20, 20, 19], # Dataset 6
    [20, 21, 19], # Dataset 7
    [19, 20, 19], # Dataset 8
    [21, 20, 18], # Dataset 9
    [20, 21, 20]  # Dataset 10

    # ... and so on for all 10 datasets
]

num_datasets = len(data)
phase_functions = ['Rayleigh', 'Henyey-Greenstein', 'Cornette-Shanks']

# Set the positions of the bars on the x-axis
ind = np.arange(num_datasets)

# Set the width of each bar
width = 0.2

fig, ax = plt.subplots()

# Generate a color for each phase function
colors = ['r', 'g', 'b']

# Create bars for each phase function
for i, phase_function in enumerate(phase_functions):
    # Extract the frame times for the i-th phase function across all datasets
    frame_times = [dataset[i] for dataset in data]
    # Create offset for the grouped bar chart
    offset = (i - len(phase_functions) / 2) * width + width / 2
    # Plot the bars for this phase function
    ax.bar(ind + offset, frame_times, width, label=f'{phase_function}', color=colors[i])

# Add some labels and title
ax.set_ylabel('Average Frame Time (ms)')
ax.set_title('Average Frame Time by Dataset and Phase Function')
ax.set_xticks(ind)
ax.set_xticklabels([f'Cloud0{i+1}' for i in range(num_datasets)])
ax.legend()

# Show the plot
plt.show()